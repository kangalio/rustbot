use std::collections::HashMap;

struct User {
    id: String,
    name: String,
    star_count: u8,
    last_star_timestamp: UnixTimestamp,

    score: u64,
}

struct CombinedLeaderboard {
    all_users: Vec<User>,
    event: u16,
}

struct RawData {
    event: u16,
    members: HashMap<String, RawDataMember>,
}

struct RawDataMember {
    name: String,
    last_star_timestamp: u64,
    star_count: u8,
    star_timestamps: HashMap<(Day, Part), UnixTimestamp>,
}

enum Part {
    Part1,
    Part2,
}

struct Day(u8);

struct UnixTimestamp(u64);

fn get_leaderboard_data() -> CombinedLeaderboard {
}
