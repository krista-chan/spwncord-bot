#![feature(allocator_api)]

use spwn::run_spwn;
use std::{
    error::Error,
    sync::Arc,
    time::{Instant, SystemTime},
};

use dotenv::dotenv;
use futures::StreamExt;
use std::time::UNIX_EPOCH;
use twilight_embed_builder::{EmbedBuilder, EmbedFieldBuilder};
use twilight_gateway::{cluster::ShardScheme, Cluster, Event, Intents};
use twilight_http::Client;
use twilight_model::{
    application::{
        callback::InteractionResponse,
        command::{ChoiceCommandOptionData, CommandOption, CommandType},
        interaction::application_command::CommandOptionValue,
    },
    channel::message::MessageFlags,
    id::Id,
};
use twilight_util::builder::{command::CommandBuilder, CallbackDataBuilder};

const APP_ID: u64 = 944228600016674867;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    dotenv()?;
    let token = std::env::var("BOT_TOKEN")?;
    let scheme = ShardScheme::Auto;

    let (cluster, mut events) = Cluster::builder(token.to_owned(), Intents::empty())
        .shard_scheme(scheme)
        .build()
        .await?;
    let cluster = Arc::new(cluster);

    let cluster_spawn = Arc::clone(&cluster);

    tokio::spawn(async move {
        cluster_spawn.up().await;
    });

    let client = Arc::new(Client::new(token));

    register_commands(Arc::clone(&client)).await?;

    while let Some((shard_id, event)) = events.next().await {
        tokio::spawn(handle_event(shard_id, event, Arc::clone(&client)));
    }

    Ok(())
}

async fn register_commands(client: Arc<Client>) -> Result<(), Box<dyn Error + Send + Sync>> {
    let commands = &[
        CommandBuilder::new(
            "ping".into(),
            String::from("Check the API latency."),
            CommandType::ChatInput,
        )
        .build(),
        CommandBuilder::new(
            "eval".into(),
            "Evaluate some spwn code".into(),
            CommandType::ChatInput,
        )
        .option(CommandOption::String(ChoiceCommandOptionData {
            name: "source".into(),
            description: "Source to parse".into(),
            required: true,
            ..Default::default()
        }))
        .build(),
    ];

    client
        .interaction(Id::new(APP_ID))
        .set_global_commands(commands)
        .exec()
        .await?;

    Ok(())
}

async fn handle_event(
    shard_id: u64,
    event: Event,
    client: Arc<Client>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match event {
        Event::InteractionCreate(interaction) => match interaction.0 {
            twilight_model::application::interaction::Interaction::ApplicationCommand(cmd) => {
                match &*cmd.data.name {
                    "ping" => {
                        let now = Instant::now();
                        let sys_time = SystemTime::now();
                        let cmd_sent = ((cmd.id.get() >> 22) + 1420070400000) as u128;
                        let ping_time = sys_time.duration_since(UNIX_EPOCH)?.as_millis() - cmd_sent;

                        let res = InteractionResponse::DeferredChannelMessageWithSource(
                            CallbackDataBuilder::new()
                                .flags(MessageFlags::EPHEMERAL)
                                .build(),
                        );

                        client
                            .interaction(Id::new(APP_ID))
                            .interaction_callback(cmd.id, &cmd.token, &res)
                            .exec()
                            .await?;

                        let elapsed = now.elapsed().as_millis();

                        let pong_field = EmbedFieldBuilder::new(
                            "Command latency",
                            format!("<:stars:928355947301208104> {}ms", ping_time),
                        )
                        .build();
                        let rtt_field = EmbedFieldBuilder::new(
                            "Roundtrip time",
                            format!("<:cp:928355939650781194> {}ms", elapsed),
                        )
                        .build();

                        let embed = EmbedBuilder::new()
                            .title("Ping")
                            .field(pong_field)
                            .field(rtt_field)
                            .color(0x3F_59_8C)
                            .build()?;

                        client
                            .interaction(Id::new(APP_ID))
                            .update_interaction_original(&cmd.token)
                            .embeds(Some(&[embed]))?
                            .exec()
                            .await?;
                    }

                    "eval" => {
                        let res = InteractionResponse::DeferredChannelMessageWithSource(
                            CallbackDataBuilder::new()
                                .flags(MessageFlags::LOADING)
                                .content("Running...".into())
                                .build(),
                        );

                        client
                            .interaction(Id::new(APP_ID))
                            .interaction_callback(cmd.id, &cmd.token, &res)
                            .exec()
                            .await?;

                        let src = &cmd.data.options[0].value;

                        if let CommandOptionValue::String(spwn_source) = src {
                            let embed = match run_spwn(spwn_source.clone(), vec![".".into()]) {
                                Ok(out) => EmbedBuilder::new()
                                    .title("SPWN")
                                    .color(0x78_F5_42)
                                    .description(format!("`SUCCESS` ```ansi\n{}\n```", out[0])),
                                Err(out) => EmbedBuilder::new()
                                    .title("SPWN | Error")
                                    .color(0xF5_42_45)
                                    .description(format!("`ERROR` ```ansi\n{}\n```", out)),
                            }
                            .build()?;

                            client
                                .interaction(Id::new(APP_ID))
                                .update_interaction_original(&cmd.token)
                                .embeds(Some(&[embed]))?
                                .exec()
                                .await?;
                        } else {
                            unreachable!()
                        }
                    }
                    _ => (),
                }
            }
            _ => (),
        },
        Event::ShardConnected(_) => {
            println!("Connected on shard {}", shard_id);
        }
        _ => {}
    }

    Ok(())
}
