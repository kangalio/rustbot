use crate::{
    api,
    commands::{Args, Result},
    db::database_connection,
    schema::tags,
};

use diesel::prelude::*;

/// Remove a key value pair from the tags.  
pub fn delete<'m>(args: Args<'m>) -> Result<()> {
    let conn = database_connection()?;
    let key = args
        .params
        .get("key")
        .ok_or("Unable to retrieve param: key")?;

    diesel::delete(tags::table.filter(tags::key.eq(key))).execute(&conn)?;
    Ok(())
}

/// Add a key value pair to the tags.  
pub fn post<'m>(args: Args<'m>) -> Result<()> {
    let conn = database_connection()?;

    let key = args
        .params
        .get("key")
        .ok_or("Unable to retrieve param: key")?;

    let value = args
        .params
        .get("value")
        .ok_or("Unable to retrieve param: value")?;

    diesel::insert_into(tags::table)
        .values((tags::key.eq(key), tags::value.eq(value)))
        .execute(&conn)?;

    Ok(())
}

/// Retrieve a value by key from the tags.  
pub fn get<'m>(args: Args<'m>) -> Result<()> {
    let conn = database_connection()?;

    let key = args.params.get("key").ok_or("unable to read params")?;

    let results = tags::table
        .filter(tags::key.eq(key))
        .load::<(i32, String, String)>(&conn)?;

    if results.is_empty() {
        api::send_reply(&args, &format!("Tag not found for `{}`", key))?;
    } else {
        api::send_reply(&args, &results[0].2)?;
    }

    Ok(())
}

/// Retrieve all tags
pub fn get_all<'m>(args: Args<'m>) -> Result<()> {
    let conn = database_connection()?;

    let results = tags::table.load::<(i32, String, String)>(&conn)?;

    if results.is_empty() {
        api::send_reply(&args, "No tags found")?;
    } else {
        let tags = &results.iter().fold(String::new(), |prev, row| {
            prev + &row.1 + ": " + &row.2 + "\n"
        });

        api::send_reply(&args, &format!("```\n{}```", &tags))?;
    }

    Ok(())
}
