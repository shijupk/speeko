use super::types::GameCommand;

pub fn map_word_to_command(word: &str) -> Option<GameCommand> {
    match word {
        "start" => Some(GameCommand::Start),
        "stop" => Some(GameCommand::Stop),
        "close" => Some(GameCommand::Close),
        "left" => Some(GameCommand::MoveLeft),
        "right" => Some(GameCommand::MoveRight),
        "yes" => Some(GameCommand::EatPrey),
        _ => None,
    }
}
