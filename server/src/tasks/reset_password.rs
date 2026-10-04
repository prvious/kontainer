//! `cargo run --bin kontainer-controller -- task auth:reset-password email:<email>`
//!
//! Reads the new password from the first line of stdin, rehashes it, and
//! signs out every session of that principal. Prints only a confirmation.

use std::io::BufRead;

use async_trait::async_trait;
use loco_rs::{
    Error, Result,
    app::AppContext,
    task::{Task, TaskInfo, Vars},
};

use crate::auth::actions::reset_password::{self, ResetPasswordError};

pub struct ResetPassword;

#[async_trait]
impl Task for ResetPassword {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "auth:reset-password".to_owned(),
            detail: "Set a new owner password (read from stdin) and sign out every session: email:<email>".to_owned(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let email = vars.cli_arg("email")?.to_owned();
        let mut line = String::new();
        std::io::stdin()
            .lock()
            .read_line(&mut line)
            .map_err(|_| Error::string("could not read the new password from stdin"))?;
        let password = line.trim_end_matches(['\n', '\r']).to_owned();
        match reset_password::execute(&ctx.db, &email, password).await {
            Ok(revoked) => {
                println!(
                    "Password updated. {revoked} active session(s) signed out; the sign-in lock for this email is cleared."
                );
                Ok(())
            }
            Err(ResetPasswordError::InvalidPassword(_)) => Err(Error::string(
                "the new password must be 12 to 128 characters",
            )),
            Err(ResetPasswordError::UnknownEmail) => {
                Err(Error::string("no account uses that email"))
            }
            Err(ResetPasswordError::Busy) => Err(Error::string(
                "the controller is busy checking passwords; nothing was changed, try again",
            )),
            Err(ResetPasswordError::Persistence) => Err(Error::string(
                "the database is unavailable; nothing was changed",
            )),
        }
    }
}
