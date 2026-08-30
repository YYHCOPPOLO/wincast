use tinycast_pure::custom_command::CustomCommand;

use crate::features::custom_commands::service::runner;

pub enum RunRequest {
    Confirm,
    Execute,
}

pub fn request_run(command: &CustomCommand) -> RunRequest {
    if command.confirm {
        RunRequest::Confirm
    } else {
        RunRequest::Execute
    }
}

pub fn execute(command: &CustomCommand) -> Result<(), String> {
    runner::run(&command.command).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_gate_is_in_coordinator() {
        let mut cmd = CustomCommand {
            id: "1".into(),
            name: "A".into(),
            command: "echo".into(),
            confirm: true,
        };
        assert!(matches!(request_run(&cmd), RunRequest::Confirm));
        cmd.confirm = false;
        assert!(matches!(request_run(&cmd), RunRequest::Execute));
    }
}
