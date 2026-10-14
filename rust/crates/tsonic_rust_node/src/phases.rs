use tsonic_rust_runtime::dispatch::{DispatchPhase, PhaseRegistration};

static REGISTRATIONS: [PhaseRegistration; 10] = [
    PhaseRegistration::new(),
    PhaseRegistration::new(),
    PhaseRegistration::new(),
    PhaseRegistration::new(),
    PhaseRegistration::new(),
    PhaseRegistration::new(),
    PhaseRegistration::new(),
    PhaseRegistration::new(),
    PhaseRegistration::new(),
    PhaseRegistration::new(),
];

#[derive(Clone, Copy)]
#[repr(usize)]
pub(crate) enum Phase {
    Background,
    RuntimeTasks,
    Signals,
    Timers,
    Http,
    Net,
    Tls,
    Watchers,
    Workers,
    Ports,
}

impl Phase {
    pub(crate) const fn registration(self) -> DispatchPhase {
        REGISTRATIONS[self as usize].phase()
    }
}

pub const BACKGROUND: DispatchPhase = Phase::Background.registration();
pub const RUNTIME_TASKS: DispatchPhase = Phase::RuntimeTasks.registration();
pub const SIGNALS: DispatchPhase = Phase::Signals.registration();
pub const TIMERS: DispatchPhase = Phase::Timers.registration();
pub const HTTP: DispatchPhase = Phase::Http.registration();
pub const NET: DispatchPhase = Phase::Net.registration();
pub const TLS: DispatchPhase = Phase::Tls.registration();
pub const WATCHERS: DispatchPhase = Phase::Watchers.registration();
pub const WORKERS: DispatchPhase = Phase::Workers.registration();
pub const PORTS: DispatchPhase = Phase::Ports.registration();

pub(crate) const ORDER: [Phase; 10] = [
    Phase::Background,
    Phase::RuntimeTasks,
    Phase::Signals,
    Phase::Timers,
    Phase::Http,
    Phase::Net,
    Phase::Tls,
    Phase::Watchers,
    Phase::Workers,
    Phase::Ports,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registrations_have_distinct_native_identity_and_explicit_order() {
        assert_ne!(core::mem::size_of::<PhaseRegistration>(), 0);
        for (index, phase) in ORDER.iter().enumerate() {
            assert_eq!(phase.registration(), REGISTRATIONS[index].phase());
            for other in &ORDER[index + 1..] {
                assert_ne!(phase.registration(), other.registration());
            }
            assert_ne!(phase.registration(), tsonic_rust_js::timers::PHASE);
        }
    }
}
