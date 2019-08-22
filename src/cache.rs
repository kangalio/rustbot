use crate::{
    commands::Result,
    db::database_connection,
    schema::{messages, roles},
};
use diesel::prelude::*;
use serenity::{model::prelude::*}; 

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
    pub(crate) fn save(name: impl Into<String>, role_id: RoleId) -> Result<()> {
        let conn = database_connection()?;

        diesel::insert_into(roles::table)
            .values((
                roles::role.eq(role_id.0.to_string()),
                roles::name.eq(name.into()),
            ))
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

    pub(crate) fn update_by_id(id: i32, role: RoleId) -> Result<()> {
        let conn = database_connection()?;

        diesel::update(roles::table.filter(roles::id.eq(id)))
            .set(roles::role.eq(role.0.to_string()))
            .execute(&conn)?;
        Ok(())
    }
}

pub(crate) fn save_or_update_role(name: &str, role: RoleId) -> Result<()> {
    match RoleIdCache::get_by_name(name)? {
        Some((id, role_id, _)) => {
            if role_id != role.0.to_string() {
                RoleIdCache::update_by_id(id, role)?;
            }
        }
        None => RoleIdCache::save(name, role)?,
    };
    Ok(())
}

pub(crate) fn save_or_update_message(name: &str, message: MessageId, channel: ChannelId) -> Result<()> {
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
