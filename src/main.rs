pub mod game;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use game::{load_route_from_path, Route};
use teloxide::{dispatching::dialogue::InMemStorage, prelude::*, utils::command::BotCommands};

type GameDialogue = Dialogue<GameState, InMemStorage<GameState>>;
type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Default)]
enum GameState {
    #[default]
    Start,
    ReceiveAnswer {
        step: usize,
        hp: usize,
        score: usize,
    },
    Completed,
}

const MAX_HP: usize = 3;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting Dora Explorer bot...");

    let route = load_route_from_path("route.json").expect("Couldn't load route.");

    let bot = Bot::from_env();

    let handler = Update::filter_message()
        .branch(dptree::entry().filter_command::<Command>().endpoint(answer))
        .branch(
            dptree::filter(|is_game_active: Arc<AtomicBool>| {
                is_game_active.load(Ordering::Relaxed)
            })
            .enter_dialogue::<Message, InMemStorage<GameState>, GameState>()
            .branch(dptree::case![GameState::Start].endpoint(start))
            .branch(
                dptree::case![GameState::ReceiveAnswer { step, hp, score }]
                    .endpoint(recieve_answer),
            ),
        );

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![
            InMemStorage::<GameState>::new(),
            route,
            Arc::new(AtomicBool::new(false))
        ])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    // Command::repl(bot, answer).await;
}

async fn recieve_answer(
    bot: Bot,
    dialogue: GameDialogue,
    msg: Message,
    (step, hp, score): (usize, usize, usize),
    route: Route,
) -> HandlerResult {
    let next_location = |msg_string: &'static str, score_mod: usize| {
        move_to_next_location(
            msg_string,
            score_mod,
            &bot,
            &dialogue,
            &msg,
            (step, hp, score),
            &route,
        )
    };
    match msg.text() {
        Some(text) => {
            if text.to_lowercase() == route.route[step].answer.to_lowercase() {
                // Correct answer case
                next_location("Correct!", hp).await?;
            } else {
                // Incorrect answer case
                let new_hp = hp - 1;

                if hp > 1 {
                    bot.send_message(msg.chat.id, format!("Wrong answer! Lives left: {new_hp}"))
                        .await?;
                    // Let the player have another try if they still have lives
                    dialogue
                        .update(GameState::ReceiveAnswer {
                            step,
                            hp: hp - 1,
                            score,
                        })
                        .await?;
                } else {
                    next_location("No lives left! Moving on...", 0).await?;
                }
            }
        }
        None => {
            bot.send_message(msg.chat.id, "Enter the answer...").await?;
        }
    };
    Ok(())
}

async fn move_to_next_location(
    msg_string: &str,
    score_mod: usize,
    bot: &Bot,
    dialogue: &GameDialogue,
    msg: &Message,
    (step, hp, score): (usize, usize, usize),
    route: &Route,
) -> HandlerResult {
    let last = step == route.route.len() - 1;

    bot.send_message(msg.chat.id, msg_string).await?;
    if !last {
        dialogue
            .update(GameState::ReceiveAnswer {
                step: step + 1,
                hp: MAX_HP,
                score: score + score_mod,
            })
            .await?;
        bot.send_message(msg.chat.id, route.route[step + 1].clue.as_str())
            .await?;
    } else {
        let final_score = score + hp;
        dialogue.update(GameState::Completed).await?;
        bot.send_message(
            msg.chat.id,
            format!("🔥 You finished 🔥\n Your final score was {final_score}"),
        )
        .await?;
    }
    Ok(())
}

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "Help with using the bot.")]
    Help,
    #[command(description = "Start the Amazing Race")]
    StartRace,
}

async fn start(bot: Bot, dialogue: GameDialogue, msg: Message, route: Route) -> HandlerResult {
    bot.send_message(msg.chat.id, route.route[0].clue.as_str())
        .await?;
    dialogue
        .update(GameState::ReceiveAnswer {
            step: 0,
            hp: MAX_HP,
            score: 0,
        })
        .await?;
    Ok(())
}

async fn answer(
    bot: Bot,
    msg: Message,
    cmd: Command,
    is_game_active: Arc<AtomicBool>,
) -> HandlerResult {
    match cmd {
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
                .await?
        }
        Command::StartRace => {
            is_game_active.fetch_or(true, Ordering::Relaxed);
            bot.send_message(
                msg.chat.id,
                "Let's start the Amazing Race! Type any text to start!",
            )
            .await?
        }
    };

    Ok(())
}
