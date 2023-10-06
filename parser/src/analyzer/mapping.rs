/// The latest property mapping for 12.8.0
pub enum ReplayPlayerProperty {
    AccountId = 0,
    AntiAbuseEnabled = 1,
    AvatarId = 2,
    CamouflageInfo = 3,
    ClanColor = 4,
    ClanId = 5,
    ClanTag = 6,
    CrewParams = 7,
    DogTag = 8,
    FragsCount = 9,
    FriendlyFireEnabled = 10,
    Id = 11,
    InvitationsEnabled = 12,
    IsAbuser = 13,
    IsAlive = 14,
    IsBot = 15,
    IsClientLoaded = 16,
    IsConnected = 17,
    IsHidden = 18,
    IsLeaver = 19,
    IsPreBattleOwner = 20,
    IsTShooter = 21,
    KilledBuildingsCount = 22,
    IsCookie = 23,
    MaxHealth = 24,
    Name = 25,
    PlayerMode = 26,
    PreBattleIdOnStart = 27,
    PreBattleSign = 28,
    PreBattleId = 29,
    Realm = 30,
    ShipComponents = 31,
    ShipConfigDump = 32,
    ShipId = 33,
    ShipParamsId = 34,
    SkinId = 35,
    TeamId = 36,
    TtkStatus = 37,
}

impl Into<i64> for ReplayPlayerProperty {
    fn into(self) -> i64 {
        self as i64
    }
}
