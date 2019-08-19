use serenity::{model::prelude::*, prelude::*, Client};
use std::collections::HashMap;

pub(crate) struct MessageCache;

impl TypeMapKey for MessageCache {
    type Value = HashMap<String, (Message, ChannelId)>;
}

impl MessageCache {
    pub(crate) fn init(client: &mut Client) {
        let mut data = client.data.write();
        data.insert::<Self>(HashMap::new());
    }

    pub(crate) fn save(cx: &Context, name: impl Into<String>, msg: (Message, ChannelId)) {
        let mut data = cx.data.write();
        let store = data
            .get_mut::<Self>()
            .expect("Unable to access message store.  ");
        store.insert(name.into(), msg);
    }
}

pub(crate) struct RoleIdCache;

impl TypeMapKey for RoleIdCache {
    type Value = HashMap<String, RoleId>;
}

impl RoleIdCache {
    pub(crate) fn init(client: &mut Client) {
        let mut data = client.data.write();
        data.insert::<Self>(HashMap::new());
    }

    pub(crate) fn save(cx: &Context, name: impl Into<String>, role_id: RoleId) {
        let mut data = cx.data.write();
        let store = data
            .get_mut::<Self>()
            .expect("Unable to access message store.  ");
        store.insert(name.into(), role_id);
    }
}
