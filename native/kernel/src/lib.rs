//! Pure turn scheduler. Message objects, effects and callbacks belong to the host.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Steering,
    Prepare,
    Request,
    Tools,
    TruncatedTools,
    Finish,
    FollowUp,
    End,
}

impl Action {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Steering => "steering",
            Self::Prepare => "prepare",
            Self::Request => "request",
            Self::Tools => "tools",
            Self::TruncatedTools => "truncated_tools",
            Self::Finish => "finish",
            Self::FollowUp => "follow_up",
            Self::End => "end",
        }
    }
}

#[derive(Debug)]
pub struct LoopDriver {
    action: Action,
    initial: bool,
    failed: bool,
    more_tools: bool,
    explicit_continuation: bool,
}

impl Default for LoopDriver {
    fn default() -> Self {
        Self {
            action: Action::Steering,
            initial: true,
            failed: false,
            more_tools: false,
            explicit_continuation: false,
        }
    }
}

impl LoopDriver {
    pub fn action(&self) -> Action {
        self.action
    }

    /// Invalid feedback leaves the state unchanged, so failures cannot replay an effect.
    pub fn advance(&mut self, feedback: &str) -> Result<Action, String> {
        self.action = match (self.action, feedback) {
            (Action::Steering, "empty" | "messages") if self.initial => {
                self.initial = false;
                Action::Request
            }
            (Action::Steering, "empty" | "messages") => {
                if self.more_tools || feedback == "messages" {
                    self.explicit_continuation = false;
                    Action::Prepare
                } else {
                    Action::FollowUp
                }
            }
            (Action::Prepare, "prepared") => Action::Request,
            (
                Action::Request,
                "response" | "response_error" | "tool_calls" | "truncated_tool_calls",
            ) => {
                self.failed = feedback == "response_error";
                self.more_tools = false;
                match feedback {
                    "tool_calls" => Action::Tools,
                    "truncated_tool_calls" => Action::TruncatedTools,
                    _ => Action::Finish,
                }
            }
            (Action::Tools | Action::TruncatedTools, "tools_done" | "tools_terminate") => {
                self.more_tools = feedback == "tools_done";
                Action::Finish
            }
            (Action::Finish, "auto" | "continue" | "end") => {
                if self.failed || feedback == "end" {
                    Action::End
                } else {
                    self.explicit_continuation = feedback == "continue";
                    Action::Steering
                }
            }
            (Action::FollowUp, "empty" | "messages") => {
                if feedback == "messages" || self.explicit_continuation {
                    self.explicit_continuation = false;
                    Action::Prepare
                } else {
                    Action::End
                }
            }
            _ => {
                return Err(format!(
                    "Invalid loop feedback {feedback:?} during {:?}",
                    self.action
                ));
            }
        };
        Ok(self.action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_and_failure_do_not_schedule_extra_requests() {
        let mut driver = LoopDriver::default();
        for feedback in ["empty", "response", "auto", "empty", "empty"] {
            driver.advance(feedback).unwrap();
        }
        assert_eq!(driver.action(), Action::End);
        for decision in ["auto", "continue", "end"] {
            let mut driver = LoopDriver::default();
            for feedback in ["messages", "response_error", decision] {
                driver.advance(feedback).unwrap();
            }
            assert_eq!(driver.action(), Action::End);
        }
    }

    #[test]
    fn natural_work_satisfies_explicit_continuation_once() {
        for response in ["tool_calls", "truncated_tool_calls"] {
            let mut driver = LoopDriver::default();
            for feedback in ["empty", response, "tools_done", "continue", "empty"] {
                driver.advance(feedback).unwrap();
            }
            assert_eq!(driver.action(), Action::Prepare);
            for feedback in ["prepared", "response", "auto", "empty", "empty"] {
                driver.advance(feedback).unwrap();
            }
            assert_eq!(driver.action(), Action::End);
        }
    }

    #[test]
    fn terminating_tools_still_allow_follow_up_or_explicit_continuation() {
        for (decision, follow_up, expected) in [
            ("auto", "empty", Action::End),
            ("auto", "messages", Action::Prepare),
            ("continue", "empty", Action::Prepare),
        ] {
            let mut driver = LoopDriver::default();
            for feedback in [
                "empty",
                "tool_calls",
                "tools_terminate",
                decision,
                "empty",
                follow_up,
            ] {
                driver.advance(feedback).unwrap();
            }
            assert_eq!(driver.action(), expected);
        }
    }

    #[test]
    fn invalid_feedback_is_atomic_and_end_is_terminal() {
        let mut driver = LoopDriver::default();
        assert!(driver.advance("prepared").is_err());
        assert_eq!(driver.action(), Action::Steering);
        for feedback in ["empty", "response", "end"] {
            driver.advance(feedback).unwrap();
        }
        assert!(driver.advance("messages").is_err());
        assert_eq!(driver.action(), Action::End);
    }
}
