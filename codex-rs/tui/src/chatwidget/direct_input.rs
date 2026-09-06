#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum DirectInputMode {
    #[default]
    Writable,
    ParentOwned,
    ActiveWriterReadOnly,
}

impl DirectInputMode {
    pub(crate) fn is_blocked(self) -> bool {
        self != Self::Writable
    }

    pub(crate) fn blocked_message(self) -> Option<&'static str> {
        match self {
            Self::Writable => None,
            Self::ParentOwned => {
                Some("This sub-agent is controlled by its parent. Direct input is disabled.")
            }
            Self::ActiveWriterReadOnly => Some(
                "Direct input is disabled for this thread. Use /fork to continue in a new thread.",
            ),
        }
    }

    pub(crate) fn allows_command_during_task(
        self,
        command: crate::slash_command::SlashCommand,
    ) -> bool {
        command == crate::slash_command::SlashCommand::Resume && self != Self::Writable
            || command == crate::slash_command::SlashCommand::Fork
                && self == Self::ActiveWriterReadOnly
    }
}
