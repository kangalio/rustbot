use crate::{
    commands::Result,
    db::database_connection,
    schema::{messages, roles, users},
};
use diesel::prelude::*;
use serenity::model::prelude::*;

pub(crate) struct MessageCache;

impl MessageCache {
    pub(crate) fn save(name: impl Into<String>, msg: MessageId, channel: ChannelId) -> Result<()> {
        let conn = database_connection()?;

        diesel::insert_into(messages::table)
            .values((
                messages::name.eq(name.into()),
                messages::message.eq(msg.0.to_string()),
                messages::channel.eq(channel.0.to_string()),
            ))
            .execute(&conn)?;
        Ok(())
    }

    pub(crate) fn get_by_name(
        name: impl Into<String>,
    ) -> Result<Option<(i32, String, String, String)>> {
        let conn = database_connection()?;

        Ok(messages::table
            .filter(messages::name.eq(name.into()))
            .load::<(i32, String, String, String)>(&conn)?
            .into_iter()
            .nth(0))
    }

    pub(crate) fn update_by_id(id: i32, message: MessageId, channel: ChannelId) -> Result<()> {
        let conn = database_connection()?;

        diesel::update(messages::table.filter(messages::id.eq(id)))
            .set((
                messages::message.eq(message.0.to_string()),
                messages::channel.eq(channel.0.to_string()),
            ))
            .execute(&conn)?;
        Ok(())
    }
}

pub(crate) struct RoleIdCache;

impl RoleIdCache {
    pub(crate) fn save(name: impl Into<String>, role_id: &str) -> Result<()> {
        let conn = database_connection()?;

        diesel::insert_into(roles::table)
            .values((roles::role.eq(role_id), roles::name.eq(name.into())))
            .execute(&conn)?;
        Ok(())
    }

    pub(crate) fn get_by_name(name: impl Into<String>) -> Result<Option<(i32, String, String)>> {
        let conn = database_connection()?;

        Ok(roles::table
            .filter(roles::name.eq(name.into()))
            .load::<(i32, String, String)>(&conn)?
            .into_iter()
            .nth(0))
    }

    pub(crate) fn update_by_id(id: i32, role: &str) -> Result<()> {
        let conn = database_connection()?;

        diesel::update(roles::table.filter(roles::id.eq(id)))
            .set(roles::role.eq(role))
            .execute(&conn)?;
        Ok(())
    }
}

pub(crate) struct UserIdCache;

impl UserIdCache {
    pub(crate) fn save(name: impl Into<String>, user_id: &str) -> Result<()> {
        let conn = database_connection()?;

        diesel::insert_into(users::table)
            .values((users::user_id.eq(user_id), users::name.eq(name.into())))
            .execute(&conn)?;
        Ok(())
    }

    pub(crate) fn get_by_name(name: impl Into<String>) -> Result<Option<(i32, String, String)>> {
        let conn = database_connection()?;

        Ok(users::table
            .filter(users::name.eq(name.into()))
            .load::<(i32, String, String)>(&conn)?
            .into_iter()
            .nth(0))
    }

    pub(crate) fn update_by_id(id: i32, name: &str, user_id: &str) -> Result<()> {
        let conn = database_connection()?;

        diesel::update(users::table.filter(users::id.eq(id)))
            .set((users::name.eq(name), users::user_id.eq(user_id)))
            .execute(&conn)?;
        Ok(())
    }
}

pub(crate) fn save_or_update_role(name: &str, role: String) -> Result<()> {
    match RoleIdCache::get_by_name(name)? {
        Some((id, role_id, _)) => {
            if role_id != role {
                RoleIdCache::update_by_id(id, &role)?;
            }
        }
        None => RoleIdCache::save(name, &role)?,
    };
    Ok(())
}

pub(crate) fn save_or_update_message(
    name: &str,
    message: MessageId,
    channel: ChannelId,
) -> Result<()> {
    match MessageCache::get_by_name(name)? {
        Some((id, _name, msg, chan)) => {
            if msg != message.0.to_string() || chan != channel.0.to_string() {
                MessageCache::update_by_id(id, message, channel)?;
            }
        }
        None => MessageCache::save(name, message, channel)?,
    };
    Ok(())
}

pub(crate) fn save_or_update_user(name: &str, user_id: &UserId) -> Result<()> {
    let user_id = user_id.0.to_string();
    match UserIdCache::get_by_name(name)? {
        Some((id, cached_name, cached_user_id)) => {
            if name != cached_name || cached_user_id != user_id {
                UserIdCache::update_by_id(id, name, &user_id)?;
            }
        }
        None => UserIdCache::save(name, &user_id)?,
    };
    Ok(())
}
