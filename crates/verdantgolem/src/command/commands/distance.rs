use crate::TextComponent;

use crate::command::args::FindArg;
use crate::command::args::position_3d::Position3DArgumentConsumer;

use crate::command::args::ConsumedArgs;
use crate::command::tree::CommandTree;
use crate::command::tree::builder::argument;
use crate::command::{CommandExecutor, CommandResult, CommandSender};
use verdantgolem_util::math::vector3::Vector3;

const NAMES: [&str; 1] = ["distance"];

const DESCRIPTION: &str = "Measures the distance between two positions.";

const ARG_FROM: &str = "from";
const ARG_TO: &str = "to";

struct DistanceExecutor;

impl CommandExecutor for DistanceExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        _server: &'a crate::server::Server,
        args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let from: Vector3<f64> = Position3DArgumentConsumer::find_arg(args, ARG_FROM)?;
            let to: Vector3<f64> = Position3DArgumentConsumer::find_arg(args, ARG_TO)?;

            let delta = to - from;
            let manhattan = delta.x.abs() + delta.y.abs() + delta.z.abs();
            let euclidean = delta.length();

            sender
                .send_message(TextComponent::text(format!(
                    "From {from:?} to {to:?}:\n\
                     Manhattan (x+y+z): {manhattan}\n\
                     Euclidean: {euclidean}"
                )))
                .await;

            Ok(manhattan as i32)
        })
    }
}

pub fn init_command_tree() -> CommandTree {
    CommandTree::new(NAMES, DESCRIPTION).then(
        argument(ARG_FROM, Position3DArgumentConsumer)
            .then(argument(ARG_TO, Position3DArgumentConsumer).execute(DistanceExecutor)),
    )
}
