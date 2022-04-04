use std::env;

use serenity::{
    async_trait,
    model::{
        channel::Message,
        gateway::Ready,
        id::GuildId,
        interactions::{
            application_command::{
                ApplicationCommandInteractionDataOptionValue, ApplicationCommandOptionType,
            },
            Interaction, InteractionResponseType,
        },
    },
    prelude::*,
};
use songbird::SerenityInit;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            let content = match command.data.name.as_str() {
                "join" => {
                    let guild_id = command.guild_id.expect("invalid guild");
                    let guild = guild_id
                        .to_guild_cached(&ctx.cache)
                        .expect("no guild found in cache");

                    let channel_id = guild
                        .voice_states
                        .get(&command.user.id)
                        .and_then(|voice_state| voice_state.channel_id);

                    let connect_to = match channel_id {
                        Some(channel) => channel,
                        None => {
                            check_msg(
                                command
                                    .channel_id
                                    .send_message(&ctx.http, |f| {
                                        f.content("not in a voice channel")
                                    })
                                    .await,
                            );
                            return;
                        }
                    };

                    let manager = songbird::get(&ctx)
                        .await
                        .expect("Songbird Voice client placed in at initialisation.")
                        .clone();

                    let _handler = manager.join(guild_id, connect_to).await;

                    "joined voice channel".to_string()
                }
                "play" => {
                    let options = command
                        .data
                        .options
                        .get(0)
                        .expect("Expected attachment option")
                        .resolved
                        .as_ref()
                        .expect("Expected attachment object");
                    let guild_id = command.guild_id.expect("invalid guild");

                    if let ApplicationCommandInteractionDataOptionValue::Attachment(attachment) =
                        options
                    {
                        let manager = songbird::get(&ctx)
                            .await
                            .expect("Failed to get songbird manager")
                            .clone();
                        let handler_lock = match manager.get(guild_id) {
                            Some(handler) => handler,
                            None => {
                                check_msg(
                                    command
                                        .channel_id
                                        .send_message(&ctx.http, |f| {
                                            f.content("not in voice channel and its line 87s fault")
                                        })
                                        .await,
                                );

                                return;
                            }
                        };
                        let mut handler = handler_lock.lock().await;
                        let source = match songbird::ytdl(&attachment.url).await {
                            Ok(source) => source,
                            Err(why) => {
                                println!("Err starting source: {:?}", why);

                                check_msg(
                                    command
                                        .channel_id
                                        .send_message(&ctx.http, |f| f.content("ffmpeg error"))
                                        .await,
                                );
                                return;
                            }
                        };

                        handler
                            .play_source(source)
                            .set_volume(1.0f32)
                            .expect("error playing track");
                    } else {
                        check_msg(
                            command
                                .channel_id
                                .send_message(&ctx.http, |f| {
                                    f.content("please provide a valid attachment")
                                })
                                .await,
                        );
                    }
                    "playing song".to_string()
                }
                "stop" => {
                    let guild_id = command.guild_id.expect("invalid guild");
                    let manager = songbird::get(&ctx)
                        .await
                        .expect("Failed to get songbird manager")
                        .clone();
                    manager.leave(guild_id).await.expect("failed to leave vc");
                    "bye :)".to_string()
                }
                _ => "not implemented :(".to_string(),
            };

            if let Err(why) = command
                .create_interaction_response(&ctx.http, |response| {
                    response
                        .kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|message| message.content(content))
                })
                .await
            {
                println!("Cannot respond to slash command: {}", why);
                return;
            }
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);

        let guild_id = GuildId(
            env::var("GUILD_ID")
                .expect("Expected GUILD_ID in environment")
                .parse()
                .expect("GUILD_ID must be an integer"),
        );

        let commands = GuildId::set_application_commands(&guild_id, &ctx.http, |commands| {
            commands
                .create_application_command(|command| {
                    command
                        .name("join")
                        .description("join a voice channel")
                        .create_option(|option| {
                            option
                                .name("id")
                                .description("the channel id to join")
                                .kind(ApplicationCommandOptionType::Channel)
                        })
                })
                .create_application_command(|command| {
                    command
                        .name("play")
                        .description("Play a song")
                        .create_option(|option| {
                            option
                                .name("file")
                                .description("The file to play")
                                .kind(ApplicationCommandOptionType::Attachment)
                                .required(true)
                        })
                })
        })
        .await;

        println!(
            "I now have the following guild slash commands: {:#?}",
            commands
        );
    }
}

#[tokio::main]
async fn main() {
    if let Err(i) = dotenv::dotenv() {
        println!("{}", i.to_string())
    }
    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    // The Application Id is usually the Bot User Id.
    let application_id: u64 = env::var("APPLICATION_ID")
        .expect("Expected an application id in the environment")
        .parse()
        .expect("application id is not a valid id");

    // Build our client.
    let mut client = Client::builder(token)
        .event_handler(Handler)
        .application_id(application_id)
        .register_songbird()
        .await
        .expect("Error creating client");

    // Finally, start a single shard, and start listening to events.
    //
    // Shards will automatically attempt to reconnect, and will perform
    // exponential backoff until it reconnects.
    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
/// Checks that a message successfully sent; if not, then logs why to stdout. Stolen from Songbird's example
fn check_msg(result: serenity::Result<Message>) {
    if let Err(why) = result {
        println!("Error sending message: {:?}", why);
    }
}
