table! {
    messages (id) {
        id -> Integer,
        name -> Text,
        message -> Text,
        channel -> Text,
    }
}

table! {
    roles (id) {
        id -> Integer,
        role -> Text,
        name -> Text,
    }
}

table! {
    tags (id) {
        id -> Integer,
        key -> Text,
        value -> Text,
    }
}

allow_tables_to_appear_in_same_query!(messages, roles, tags,);
