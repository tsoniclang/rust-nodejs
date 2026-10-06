use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::{register_ready, Failure, OtherFailure};
use crate::background::{background_task_budget, BackgroundTasks};

#[test]
fn composed_background_domains_use_one_ready_order_and_retain_uninvoked_callbacks() {
    use crate::dispatch::{DispatchContexts, DispatchEnd, DispatchPhase};
    let first = BackgroundTasks::<Failure>::new();
    let second = BackgroundTasks::<OtherFailure>::new();
    let observed = Rc::new(RefCell::new(Vec::new()));
    let recorded = observed.clone();
    register_ready(second.scheduling_source(), move || {
        recorded.borrow_mut().push(0);
        Ok(())
    });
    assert!(second.prepare(DispatchPhase::Background).is_ok());
    let recorded = observed.clone();
    register_ready(first.scheduling_source(), move || {
        recorded.borrow_mut().push(1);
        Ok(())
    });
    assert!(first.prepare(DispatchPhase::Background).is_ok());
    let expected = Rc::new(Cell::new(9_007_199_254_740_993));
    let failure = expected.clone();
    register_ready(second.scheduling_source(), move || {
        Err(OtherFailure(Failure::Payload(failure)))
    });
    assert!(second.prepare(DispatchPhase::Background).is_ok());
    let recorded = observed.clone();
    register_ready(first.scheduling_source(), move || {
        recorded.borrow_mut().push(3);
        Ok(())
    });
    let contexts = crate::dispatch::prepend(
        &first,
        crate::dispatch::prepend(&second, DispatchEnd::<Failure>::new()),
    );
    let returned = crate::dispatch::poll_phase(&contexts, DispatchPhase::Background)
        .err()
        .expect("source callback must fail");
    let Failure::Payload(returned) = returned else {
        panic!("exact source error domain must survive");
    };
    assert!(Rc::ptr_eq(&returned, &expected));
    assert_eq!(returned.get(), 9_007_199_254_740_993);
    assert_eq!(&*observed.borrow(), &[0, 1]);
    assert!(first.has_pending_work());
    assert!(!second.has_pending_work());
    assert!(crate::dispatch::poll_phase(&contexts, DispatchPhase::Background).is_ok());
    assert_eq!(&*observed.borrow(), &[0, 1, 3]);
    assert_eq!(background_task_budget().pending(), 0);
}

#[test]
fn composed_phase_prepares_every_domain_before_reentrant_dispatch() {
    use crate::dispatch::{DispatchEnd, DispatchPhase};
    let first = BackgroundTasks::<Failure>::new();
    let second = Rc::new(BackgroundTasks::<OtherFailure>::new());
    let reentrant = second.clone();
    let observed = Rc::new(Cell::new(0));
    let recorded = observed.clone();
    register_ready(first.scheduling_source(), move || {
        register_ready(reentrant.scheduling_source(), move || {
            recorded.set(2);
            Ok(())
        });
        assert!(reentrant.poll().is_err());
        Ok(())
    });
    register_ready(second.scheduling_source(), || {
        Err(OtherFailure(Failure::Payload(Rc::new(Cell::new(1)))))
    });
    let contexts = crate::dispatch::prepend(
        &first,
        crate::dispatch::prepend(&*second, DispatchEnd::<Failure>::new()),
    );
    assert!(crate::dispatch::poll_phase(&contexts, DispatchPhase::Background).is_ok());
    assert_eq!(observed.get(), 0);
    assert!(second.has_pending_work());
    assert!(crate::dispatch::poll_phase(&contexts, DispatchPhase::Background).is_ok());
    assert_eq!(observed.get(), 2);
    assert_eq!(background_task_budget().pending(), 0);
}

#[test]
fn native_node_driver_transports_exact_component_background_errors() {
    use crate::dispatch::DispatchEnd;
    let first = BackgroundTasks::<Failure>::new();
    let second = BackgroundTasks::<OtherFailure>::new();
    let expected = Rc::new(Cell::new(9_007_199_254_740_993));
    let failure = expected.clone();
    first
        .spawn(|| Ok(()), move |_| Err(Failure::Payload(failure)))
        .unwrap();
    let observed = Rc::new(Cell::new(0));
    let recorded = observed.clone();
    second
        .spawn(
            || Ok(()),
            move |_| {
                recorded.set(1);
                Ok(())
            },
        )
        .unwrap();
    let contexts = crate::dispatch::prepend(
        &first,
        crate::dispatch::prepend(&second, DispatchEnd::<Failure>::new()),
    );
    let returned = crate::run_with_contexts(&contexts)
        .err()
        .expect("actual native callback must fail");
    let Failure::Payload(returned) = returned else {
        panic!("native driver must not format source errors");
    };
    assert!(Rc::ptr_eq(&returned, &expected));
    assert_eq!(returned.get(), 9_007_199_254_740_993);
    assert!(crate::run_with_contexts(&contexts).is_ok());
    assert_eq!(observed.get(), 1);
    assert_eq!(background_task_budget().pending(), 0);
}

#[test]
fn native_async_driver_drops_its_root_future_on_exact_callback_failure() {
    use crate::dispatch::DispatchEnd;
    struct PendingRoot(Rc<Cell<usize>>);
    impl std::future::Future for PendingRoot {
        type Output = usize;
        fn poll(
            self: std::pin::Pin<&mut Self>,
            _context: &mut std::task::Context<'_>,
        ) -> std::task::Poll<usize> {
            std::task::Poll::Pending
        }
    }
    impl Drop for PendingRoot {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let root = BackgroundTasks::<Failure>::new();
    let expected = Rc::new(Cell::new(9_007_199_254_740_993));
    let failure = expected.clone();
    root.spawn(|| Ok(()), move |_| Err(Failure::Payload(failure)))
        .unwrap();
    let released = Rc::new(Cell::new(0));
    let contexts = crate::dispatch::prepend(&root, DispatchEnd::<Failure>::new());
    let returned = crate::block_on_with_contexts(PendingRoot(released.clone()), contexts)
        .err()
        .expect("native background callback must interrupt the root future");
    let Failure::Payload(returned) = returned else {
        panic!("source error type must survive native async driving");
    };
    assert!(Rc::ptr_eq(&returned, &expected));
    assert_eq!(released.get(), 1);
    assert_eq!(background_task_budget().pending(), 0);
}
