use serde_json::Map;
use humoreic::update_message;
use humoreic::create_message;
use dotenv::dotenv;
use humoreic::create_admin;
use humoreic::create_ban;
use humoreic::establish_connection;
use humoreic::is_admin;
use humoreic::is_banned;
use humoreic::rm_ban;
use humoreic::PgPool;
use humoreic::{create_guild, get_guild, get_guilds, find_message};
use regex::Regex;
use std::env;
use std::collections::HashMap;
use serenity::builder::CreateEmbed;
use serde_json::json;

use serenity::{
    async_trait,
    model::{channel::{Message, Reaction}, gateway::Ready, id::ChannelId},
    prelude::*,
};

use serenity::framework::standard::{
    macros::{command, group},
    CommandResult, StandardFramework,
};

struct DBConnection;
impl TypeMapKey for DBConnection {
    type Value = PgPool;
}

#[group]
#[commands(ping, setup, admin, addadmin, ban, unban)]
struct General;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }

    async fn reaction_add(&self, ctx: Context, reaction: Reaction) {
        let data = ctx.data.read().await;
        let pool = data.get::<DBConnection>().unwrap();
        let conn = pool.get().unwrap();

        let message = find_message(&conn, *reaction.message_id.as_u64() as i64, *reaction.guild_id.unwrap().as_u64() as i64);
        let guilds = get_guilds(&conn);
        let r = reaction.emoji.to_string();

        let embeds = message.embed_ids.as_object().unwrap();
        let reactions = message.reactions.as_object().unwrap();
        let mut new_reactions = Map::new();
        
        if let Some(reaction_data) = reactions.get(&r) {
            let reaction_list = reaction_data.as_array().unwrap();
            if reaction_list.contains(&json!(*reaction.user_id.unwrap().as_u64() as i64)) {
                reaction.delete(&ctx.http).await.expect("Delete it boi");
                return;
            }

            let mut new_list = reaction_list.clone();
            new_list.push(json!(*reaction.user_id.unwrap().as_u64() as i64));
            new_reactions.insert(r.clone(), json!(new_list));
        }

        for (reac, list) in reactions {
            if *reac != r {
                new_reactions.insert(reac.clone(), list.clone());
            }
        }

        update_message(&conn, message.id, new_reactions);

        for g in &guilds {
            let message_id = embeds.get(&g.id.to_string()).unwrap().as_u64().unwrap();
            let channel = ChannelId(g.channel_id as u64);
            let mut msg = channel.message(&ctx.http, message_id).await.expect("Work bitch!");
            let mut fake_embed = msg.embeds.remove(0);
            fake_embed.fields = vec![];

            let mut embed = CreateEmbed::from(fake_embed);
            embed.field(&r, "1", true);

            channel.edit_message(&ctx.http, message_id, |edit| edit.embed(|e| {
                *e = embed;
                e
            })).await.expect("You better edit the message, you little shit!");
        }
    }

    async fn reaction_remove(&self, ctx: Context, reaction: Reaction) {
        let data = ctx.data.read().await;
        let pool = data.get::<DBConnection>().unwrap();
        let conn = pool.get().unwrap();

        let message = find_message(&conn, *reaction.message_id.as_u64() as i64, *reaction.guild_id.unwrap().as_u64() as i64);
        println!("{}", message.id);
    }

    async fn message(&self, ctx: Context, msg: Message) {
        let data = ctx.data.read().await;
        let pool = data.get::<DBConnection>().unwrap();
        let conn = pool.get().unwrap();

        let guild_id = *msg.guild_id.unwrap().as_u64() as i64;
        let guild_data = get_guild(&conn, guild_id);
        let channel_id = *msg.channel_id.as_u64() as i64;

        let banned = is_banned(&conn, *msg.author.id.as_u64() as i64);

        if !msg.author.bot && guild_data.channel_id == channel_id {
            if banned {
                match msg.delete(&ctx.http).await {
                    Err(_) => println!("wtf bro"),
                    _ => (),
                };
                return;
            }

            let guild = msg.guild(&ctx.cache).await.unwrap();
            let guild_icon = guild.icon_url().unwrap();
            let guilds = get_guilds(&conn);
            let mut embed_ids = HashMap::new();
            let mut msg_ids = HashMap::new();

            for g in guilds {
                let image_regex = Regex::new(r"((http(s?)://)([/|.|\w|\s|-])*\.(?:jpg|gif|png))").unwrap();
                let tenor_regex = Regex::new(r"(http(s?)://)((tenor\.com.*)|(media\.giphy\.com.*)|(gph\.is.*))").unwrap();
                let video_regex = Regex::new(r"((http(s?)://)([/|.|\w|\s|-])*\.(?:mp4|webm))").unwrap();

                let channel = ChannelId(g.channel_id as u64);
                match channel
                    .send_message(&ctx.http, |m| {    
                        m.embed(|e| {
                            if image_regex.is_match(&msg.content) {
                                e.image(&msg.content);
                            } else {
                                e.description(&msg.content);
                            }

                            e.author(|a| {
                                a.name(&msg.author.name);
                                a.icon_url(&msg.author.face());

                                a
                            });

                            e.footer(|f| {
                                f.text(&guild.name);
                                f.icon_url(&guild_icon);

                                f
                            });

                            e
                        });

                        m
                    })
                    .await
                {
                    Err(_) => println!("wtf are u doing"),
                    Ok(message) => {
                        embed_ids.insert(g.id, *message.id.as_u64() as i64);
                    },
                };

                if tenor_regex.is_match(&msg.content) || video_regex.is_match(&msg.content) {
                    match channel.say(&ctx.http, &msg.content).await {
                        Err(_) => println!("brah learn to write Rust"),
                        Ok(message) => {
                            msg_ids.insert(g.id, *message.id.as_u64() as i64);
                        },
                    };
                }

                for attachment in msg.attachments.clone() {
                    if channel_id != *channel.as_u64() as i64 {
                        match channel.say(&ctx.http, attachment.url).await {
                            Err(_) => println!("brah learn to write Rust"),
                            Ok(message) => {
                                msg_ids.insert(g.id, *message.id.as_u64() as i64);
                            },
                        };
                    }
                }
            }

            // Apagar mensagem depois de enviar a todos os servers
            // Caso não tenha enviado imagens/videos
            if msg.attachments.len() == 0 {
                match msg.delete(&ctx.http).await {
                    Err(_) => println!("wtf bro"),
                    _ => (),
                };
            } else {
                msg_ids.insert(guild_id, *msg.id.as_u64() as i64);
            }

            create_message(&conn, embed_ids, msg_ids);
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let framework = StandardFramework::new()
        .configure(|c| c.prefix("☭"))
        .group(&GENERAL_GROUP);

    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    let mut client = Client::builder(&token)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Error creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<DBConnection>(establish_connection());
    }

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}

#[command]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    msg.reply(
        ctx,
        "Não quero saber! 0.75 para ti!\n".to_owned()
            + "https://scontent.flis6-1.fna.fbcdn.net/v/t1.0-9/70961516_2959"
            + "708937378994_4689648200359870464_o.jpg?_nc_cat=111&ccb=1-3&_nc_sid"
            + "=825194&_nc_ohc=D4Gg4D4CrvcAX-9A812&_nc_ht=scontent.flis6-1.fna&oh="
            + "af2a5651c2bc4c2e55affee070113262&oe=6068A3E0",
    )
    .await?;
    Ok(())
}

#[command]
async fn setup(ctx: &Context, msg: &Message) -> CommandResult {
    let data = ctx.data.read().await;
    let pool = data.get::<DBConnection>().unwrap();
    let conn = pool.get().unwrap();

    if is_admin(&conn, *msg.author.id.as_u64() as i64) {
        if let Some(guild_id) = msg.guild_id {
            create_guild(
                &conn,
                *guild_id.as_u64() as i64,
                *msg.channel_id.as_u64() as i64,
            );
        }
    }

    Ok(())
}

#[command]
async fn admin(ctx: &Context, msg: &Message) -> CommandResult {
    let data = ctx.data.read().await;
    let pool = data.get::<DBConnection>().unwrap();
    let conn = pool.get().unwrap();

    if is_admin(&conn, *msg.author.id.as_u64() as i64) {
        msg.reply(&ctx, "Ya bro, you an admin!").await?;
    } else {
        msg.reply(&ctx, "Fuck off, bro!").await?;
    }

    Ok(())
}

#[command]
async fn addadmin(ctx: &Context, msg: &Message) -> CommandResult {
    let data = ctx.data.read().await;
    let pool = data.get::<DBConnection>().unwrap();
    let conn = pool.get().unwrap();

    if is_admin(&conn, *msg.author.id.as_u64() as i64) {
        for mention in msg.mentions.iter() {
            create_admin(&conn, *mention.id.as_u64() as i64);
        }
    }

    Ok(())
}

#[command]
async fn ban(ctx: &Context, msg: &Message) -> CommandResult {
    let data = ctx.data.read().await;
    let pool = data.get::<DBConnection>().unwrap();
    let conn = pool.get().unwrap();

    if is_admin(&conn, *msg.author.id.as_u64() as i64) {
        for mention in msg.mentions.iter() {
            create_ban(&conn, *mention.id.as_u64() as i64);
        }
    }

    Ok(())
}

#[command]
async fn unban(ctx: &Context, msg: &Message) -> CommandResult {
    let data = ctx.data.read().await;
    let pool = data.get::<DBConnection>().unwrap();
    let conn = pool.get().unwrap();

    if is_admin(&conn, *msg.author.id.as_u64() as i64) {
        for mention in msg.mentions.iter() {
            rm_ban(&conn, *mention.id.as_u64() as i64);
        }
    }

    Ok(())
}
