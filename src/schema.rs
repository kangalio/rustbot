table! {
    messages (id) {
        id -> Int4,
        name -> Text,
        message -> Text,
        channel -> Text,
    }
}

table! {
    roles (id) {
        id -> Int4,
        role -> Text,
        name -> Text,
    }
}

table! {
    tags (id) {
        id -> Int4,
        key -> Text,
        value -> Text,
    }
}

table! {
    users (id) {
        id -> Int4,
        name -> Text,
        user_id -> Text,
    }
}

allow_tables_to_appear_in_same_query!(messages, roles, tags, users,);
