struct PortEmission {
    ticket: TaskTicket,
    signal: PortSignal,
    routes: Option<Rc<Vec<PortRoute>>>,
    cursor: usize,
}

impl PortSignal {
    fn event(&self) -> &'static str {
        match self {
            Self::Message(_) => "message",
            Self::Error(_) => "error",
            Self::Close => "close",
        }
    }

    fn argument(&self) -> JsValue {
        match self {
            Self::Message(value) => value.clone(),
            Self::Error(error) => JsValue::String(error.clone()),
            Self::Close => JsValue::Null,
        }
    }
}

pub struct PortFrontier {
    state: Rc<RefCell<MessagePortState>>,
    boundary: Option<TaskTicket>,
}

impl Drop for PortFrontier {
    fn drop(&mut self) {
        let mut state = self.state.borrow_mut();
        state.capture_count -= 1;
        if state.capture_count == 0 {
            state.capture_boundary = None;
        }
    }
}

impl MessagePortState {
    fn captured_boundary(&self) -> Option<TaskTicket> {
        if !self.started {
            return None;
        }
        [
            self.messages.back().map(|(ticket, _)| *ticket),
            self.errors.back().map(|(ticket, _)| *ticket),
            self.close_pending,
            self.routing.active.as_ref().map(|emission| emission.ticket),
        ]
        .into_iter()
        .flatten()
        .max()
    }

    fn prepare_emission(&mut self, boundary: TaskTicket) {
        if self.routing.active.is_some() {
            return;
        }
        let selected = if self
            .messages
            .front()
            .is_some_and(|(ticket, _)| *ticket <= boundary)
        {
            self.messages
                .pop_front()
                .map(|(ticket, value)| (ticket, PortSignal::Message(value.to_js())))
        } else if self
            .errors
            .front()
            .is_some_and(|(ticket, _)| *ticket <= boundary)
        {
            self.errors
                .pop_front()
                .map(|(ticket, error)| (ticket, PortSignal::Error(error)))
        } else if self.close_pending.is_some_and(|ticket| ticket <= boundary) {
            self.close_pending
                .take()
                .map(|ticket| (ticket, PortSignal::Close))
        } else {
            None
        };
        if let Some((ticket, signal)) = selected {
            let routes = self
                .routing
                .events
                .get(EventName::String(signal.event()))
                .cloned();
            self.routing.active = Some(PortEmission {
                ticket,
                signal,
                routes,
                cursor: 0,
            });
        }
    }

    fn next_route(&mut self, boundary: TaskTicket) -> Option<(TaskTicket, Option<PortRoute>)> {
        self.prepare_emission(boundary);
        let emission = self.routing.active.as_ref()?;
        if emission.ticket > boundary {
            return None;
        }
        let route = emission
            .routes
            .as_ref()
            .and_then(|routes| routes.get(emission.cursor))
            .cloned();
        if let Some(route) = route {
            return Some((route.owner, Some(route)));
        }
        self.routing
            .owners
            .iter()
            .next()
            .map(|owner| (owner.ticket, None))
    }
}

pub(super) fn capture_parent(state: Rc<RefCell<MessagePortState>>) -> NodeResult<PortFrontier> {
    let boundary = {
        let mut physical = state.borrow_mut();
        if physical.capture_count == 0 {
            physical.ingest_transport()?;
            physical.capture_boundary = physical.captured_boundary();
            physical.capture_count = 1;
        } else {
            physical.capture_count = physical.capture_count.checked_add(1).ok_or_else(|| {
                NodeError::new(
                    "ERR_WORKER_CAPTURE_LIMIT",
                    "message port frontier count exceeds the native range",
                )
            })?;
        }
        physical.capture_boundary
    };
    Ok(PortFrontier { state, boundary })
}

impl<E: From<NodeError> + 'static> MessagePort<E> {
    pub(super) fn capture(&self) -> Result<Option<TaskTicket>, E> {
        let captured = {
            let mut state = self.owner.state.borrow_mut();
            state.ingest_transport().map(|()| state.captured_boundary())
        };
        captured.map_err(E::from)
    }

    pub(super) fn physical_state(&self) -> Rc<RefCell<MessagePortState>> {
        Rc::clone(&self.owner.state)
    }

    pub(super) fn next_shared(&self, frontier: &PortFrontier) -> Option<u64> {
        if !Rc::ptr_eq(&self.owner.state, &frontier.state) {
            return None;
        }
        self.next_delivery(frontier.boundary)
    }

    fn next_delivery(&self, boundary: Option<TaskTicket>) -> Option<u64> {
        let mut state = self.owner.state.borrow_mut();
        let (owner, _) = state.next_route(boundary?)?;
        (owner == self.owner.reservation.ticket()).then(|| state.reservation.ticket().sequence())
    }

    pub(super) fn poll_shared(&self, frontier: &PortFrontier) -> Result<bool, E> {
        if self.next_shared(frontier).is_none() {
            return Ok(false);
        }
        self.poll_delivery(frontier.boundary)
    }

    pub(super) fn poll(&self, boundary: Option<TaskTicket>) -> Result<bool, E> {
        let mut did_work = false;
        while self.next_delivery(boundary).is_some() {
            did_work |= self.poll_delivery(boundary)?;
        }
        Ok(did_work)
    }

    fn poll_delivery(&self, boundary: Option<TaskTicket>) -> Result<bool, E> {
        let Some(boundary) = boundary else {
            return Ok(false);
        };
        let selected = self.owner.state.borrow_mut().next_route(boundary);
        let Some((owner, route)) = selected else {
            return Ok(false);
        };
        if owner != self.owner.reservation.ticket() {
            return Ok(false);
        }
        let Some(route) = route else {
            let error = {
                let state = self.owner.state.borrow();
                let emission = state
                    .routing
                    .active
                    .as_ref()
                    .expect("prepared port emission");
                if emission.cursor == 0 && matches!(emission.signal, PortSignal::Error(_)) {
                    Some(crate::events::unhandled_error(&[emission
                        .signal
                        .argument()]))
                } else {
                    None
                }
            };
            self.finish_emission();
            return error.map_or(Ok(true), |error| Err(E::from(error)));
        };
        let callback = self
            .owner
            .callbacks
            .borrow()
            .entries
            .get(route.slot)
            .filter(|entry| entry.ticket == route.ticket)
            .and_then(|entry| entry.callback.clone());
        let emission_ticket = {
            let mut state = self.owner.state.borrow_mut();
            let emission = state
                .routing
                .active
                .as_mut()
                .expect("prepared port emission");
            emission.cursor += 1;
            let emission_ticket = emission.ticket;
            if route.once && self.owner.is_registered(route.ticket, route.slot) {
                state.routing.cleanup_pending = true;
            }
            emission_ticket
        };
        if route.once {
            self.owner.retire(route.ticket, route.slot, true);
        }
        let result = callback.map_or(Ok(false), |callback| {
            callback
                .with_callback(|callback| {
                    callback.invoke_with_values(|index| {
                        if index == 0 {
                            self.owner
                                .state
                                .borrow()
                                .routing
                                .active
                                .as_ref()
                                .expect("active port callback")
                                .signal
                                .argument()
                        } else {
                            JsValue::Null
                        }
                    })
                })
                .transpose()
                .map(|result| result.is_some())
        });
        let (current, complete) =
            self.owner
                .state
                .borrow()
                .routing
                .active
                .as_ref()
                .map_or((false, false), |emission| {
                    (
                        emission.ticket == emission_ticket,
                        emission
                            .routes
                            .as_ref()
                            .is_none_or(|routes| emission.cursor >= routes.len()),
                    )
                });
        if current && (result.is_err() || complete) {
            self.finish_emission();
        }
        result
    }
}
