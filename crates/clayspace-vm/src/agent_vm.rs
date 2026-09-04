//! Whether the application is listening, and what an agent has been doing.
//!
//! The status area reads this, the menu writes it, and a consent an agent
//! needs stands here until somebody answers it. It is a ViewModel rather than
//! state in the composition root for the reason every other one is: the shell
//! draws from state and emits commands, and a modal the shell owned would be a
//! piece of the application only a window could test.
//!
//! Nothing here knows what a socket is. The server's own state arrives through
//! [`AgentViewModel::listening`], which the composition root sets.

use crate::observable::Observable;

/// A kind of operation an agent cannot do on the strength of its connection.
///
/// The same six as `clayspace_mcp::GateKind`, said again here because the
/// ViewModel layer does not depend on the agent-facing crate — that edge runs
/// the other way, and the domain of "what a person is being asked" is this
/// layer's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentGate {
    Overwrite,
    Export,
    Open,
    DiscardUnsaved,
    IrreversibleRemoval,
    Quit,
}

impl AgentGate {
    /// The word recorded in the session store.
    pub fn tag(self) -> &'static str {
        match self {
            Self::Overwrite => "sobrescrever",
            Self::Export => "exportar",
            Self::Open => "abrir",
            Self::DiscardUnsaved => "descartar",
            Self::IrreversibleRemoval => "remover",
            Self::Quit => "sair",
        }
    }
}

/// What an agent is asking permission for.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentAsk {
    /// Names this ask across the several times it is looked at.
    pub id: u64,
    pub gate: AgentGate,
    /// The operation in the words the interface uses for it.
    pub operation: String,
    /// Which client is asking, so nobody consents into the dark.
    pub client: String,
    /// The path involved, where there is one.
    pub path: Option<String>,
}

/// How an ask was answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentAnswer {
    /// Just this once.
    Yes,
    /// This and every later one of the same kind, recorded in the session
    /// store.
    Always,
    No,
}

/// Where the door stands.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Door {
    pub listening: bool,
    /// What a client connects to, empty where nothing is listening.
    pub url: String,
    /// The secret a client needs. Shown only where the person asks for it, and
    /// never in the diagnostics report.
    pub secret: String,
    pub connected: usize,
}

/// The agent-facing door, as the interface sees it.
#[derive(Debug, Default)]
pub struct AgentViewModel {
    door: Observable<Door>,
    /// The ask standing at the window, if one is.
    ask: Observable<Option<AgentAsk>>,
    /// The answer to the standing ask, until the server has read it.
    answered: Option<(u64, AgentAnswer)>,
    /// How many commands this session took from an agent.
    from_agent: Observable<u64>,
    /// How many seconds ago an agent last changed the document, where one has.
    ///
    /// Seconds rather than an `Instant`: this layer has no clock, which is
    /// what lets its rules be tested without sleeping.
    since_last: Observable<Option<u64>>,
    /// Whether the person has asked to see the secret.
    showing_access: Observable<bool>,
}

impl AgentViewModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn door(&self) -> &Door {
        self.door.get()
    }

    pub fn door_revision(&self) -> u64 {
        self.door.revision()
    }

    pub fn listening(&mut self, door: Door) {
        self.door.set_if_changed(door);
    }

    pub fn is_listening(&self) -> bool {
        self.door.get().listening
    }

    /// Whether the secret is on screen. Off by default and off again the
    /// moment the panel closes: a secret left on a screen is a secret in every
    /// screenshot taken afterwards.
    pub fn showing_access(&self) -> bool {
        *self.showing_access.get()
    }

    pub fn show_access(&mut self, showing: bool) {
        self.showing_access.set_if_changed(showing);
    }

    /// The ask standing at the window.
    pub fn ask(&self) -> Option<&AgentAsk> {
        self.ask.get().as_ref()
    }

    pub fn ask_revision(&self) -> u64 {
        self.ask.revision()
    }

    /// Raises an ask, or leaves the standing one alone where it is the same
    /// question being looked at again.
    ///
    /// Returns whether this raised a new one.
    pub fn raise(&mut self, ask: AgentAsk) -> bool {
        if self.ask.get().as_ref().map(|standing| standing.id) == Some(ask.id) {
            return false;
        }
        self.ask.set(Some(ask));
        true
    }

    /// Answers the standing ask.
    ///
    /// The answer is kept until the server reads it rather than acted on here:
    /// this layer has no session store and no connection, and a ViewModel that
    /// wrote a file would be a ViewModel a test needs a home directory for.
    pub fn answer(&mut self, answer: AgentAnswer) {
        if let Some(standing) = self.ask.get().as_ref() {
            self.answered = Some((standing.id, answer));
            self.ask.set(None);
        }
    }

    /// Takes the answer to an ask, if it has been answered.
    pub fn take_answer(&mut self, id: u64) -> Option<AgentAnswer> {
        match self.answered {
            Some((answered, answer)) if answered == id => {
                self.answered = None;
                Some(answer)
            }
            _ => None,
        }
    }

    /// Drops the standing ask without answering it — the bound was reached.
    pub fn withdraw(&mut self, id: u64) {
        if self.ask.get().as_ref().map(|ask| ask.id) == Some(id) {
            self.ask.set(None);
        }
    }

    pub fn from_agent(&self) -> u64 {
        *self.from_agent.get()
    }

    /// Records that an agent changed the document.
    pub fn acted(&mut self) {
        self.from_agent.update(|count| *count += 1);
        self.since_last.set(Some(0));
    }

    /// How long ago that was, in seconds.
    pub fn seconds_since_agent_acted(&self) -> Option<u64> {
        *self.since_last.get()
    }

    pub fn tick(&mut self, seconds: u64) {
        if let Some(since) = *self.since_last.get() {
            self.since_last.set_if_changed(Some(since + seconds));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn an_ask(id: u64) -> AgentAsk {
        AgentAsk {
            id,
            gate: AgentGate::Overwrite,
            operation: "salvar".into(),
            client: "um agente".into(),
            path: Some("/tmp/cabeça.clayspace".into()),
        }
    }

    #[test]
    fn nothing_listens_until_it_is_told_to() {
        let vm = AgentViewModel::new();
        assert!(!vm.is_listening());
        assert_eq!(vm.door().url, "");
        assert_eq!(vm.from_agent(), 0);
        assert!(vm.ask().is_none());
    }

    #[test]
    fn the_door_is_what_the_status_area_reads() {
        let mut vm = AgentViewModel::new();
        let before = vm.door_revision();
        vm.listening(Door {
            listening: true,
            url: "http://127.0.0.1:7457/mcp".into(),
            secret: "abc".into(),
            connected: 1,
        });
        assert!(vm.is_listening());
        assert_eq!(vm.door().connected, 1);
        assert_ne!(vm.door_revision(), before);
    }

    /// Reading never marks anything changed. That is the property that stops
    /// an idle application redrawing forever, and an agent polling the session
    /// must not be the thing that breaks it.
    #[test]
    fn reading_marks_nothing_changed() {
        let mut vm = AgentViewModel::new();
        vm.listening(Door {
            listening: true,
            ..Door::default()
        });
        let door = vm.door_revision();
        let ask = vm.ask_revision();
        for _ in 0..10 {
            let _ = vm.door();
            let _ = vm.ask();
            let _ = vm.from_agent();
            let _ = vm.is_listening();
        }
        assert_eq!(vm.door_revision(), door);
        assert_eq!(vm.ask_revision(), ask);
    }

    #[test]
    fn the_same_door_twice_is_not_a_change() {
        let mut vm = AgentViewModel::new();
        let door = Door {
            listening: true,
            url: "http://127.0.0.1:7457/mcp".into(),
            secret: "abc".into(),
            connected: 0,
        };
        vm.listening(door.clone());
        let revision = vm.door_revision();
        vm.listening(door);
        assert_eq!(vm.door_revision(), revision);
    }

    #[test]
    fn an_ask_stands_until_it_is_answered() {
        let mut vm = AgentViewModel::new();
        assert!(vm.raise(an_ask(1)));
        assert_eq!(vm.ask().map(|ask| ask.id), Some(1));

        // Looking at the same question again does not raise it twice.
        assert!(!vm.raise(an_ask(1)));

        vm.answer(AgentAnswer::Yes);
        assert!(vm.ask().is_none());
        assert_eq!(vm.take_answer(1), Some(AgentAnswer::Yes));
        // Read once, and gone: a second operation asks again.
        assert_eq!(vm.take_answer(1), None);
    }

    #[test]
    fn an_answer_belongs_to_the_ask_that_got_it() {
        let mut vm = AgentViewModel::new();
        vm.raise(an_ask(7));
        vm.answer(AgentAnswer::Always);
        assert_eq!(vm.take_answer(8), None);
        assert_eq!(vm.take_answer(7), Some(AgentAnswer::Always));
    }

    #[test]
    fn an_ask_nobody_answered_can_be_withdrawn() {
        let mut vm = AgentViewModel::new();
        vm.raise(an_ask(3));
        vm.withdraw(4);
        assert!(vm.ask().is_some(), "a different ask was withdrawn");
        vm.withdraw(3);
        assert!(vm.ask().is_none());
        assert_eq!(vm.take_answer(3), None);
    }

    #[test]
    fn answering_nothing_answers_nothing() {
        let mut vm = AgentViewModel::new();
        vm.answer(AgentAnswer::Yes);
        assert_eq!(vm.take_answer(1), None);
    }

    #[test]
    fn what_an_agent_did_is_attributable() {
        let mut vm = AgentViewModel::new();
        assert_eq!(vm.seconds_since_agent_acted(), None);
        vm.acted();
        vm.acted();
        assert_eq!(vm.from_agent(), 2);
        assert_eq!(vm.seconds_since_agent_acted(), Some(0));
        vm.tick(30);
        assert_eq!(vm.seconds_since_agent_acted(), Some(30));
        vm.acted();
        assert_eq!(vm.seconds_since_agent_acted(), Some(0));
    }

    #[test]
    fn the_clock_does_not_run_before_an_agent_has_acted() {
        let mut vm = AgentViewModel::new();
        vm.tick(30);
        assert_eq!(vm.seconds_since_agent_acted(), None);
    }

    #[test]
    fn the_secret_is_not_on_screen_until_it_is_asked_for() {
        let mut vm = AgentViewModel::new();
        assert!(!vm.showing_access());
        vm.show_access(true);
        assert!(vm.showing_access());
        vm.show_access(false);
        assert!(!vm.showing_access());
    }

    #[test]
    fn every_gate_has_a_tag_and_they_are_distinct() {
        let gates = [
            AgentGate::Overwrite,
            AgentGate::Export,
            AgentGate::Open,
            AgentGate::DiscardUnsaved,
            AgentGate::IrreversibleRemoval,
            AgentGate::Quit,
        ];
        let mut tags: Vec<&str> = gates.iter().map(|gate| gate.tag()).collect();
        tags.sort_unstable();
        tags.dedup();
        assert_eq!(tags.len(), gates.len());
    }
}
