use time::OffsetDateTime;

#[derive(Debug)]
pub struct StartServerInteraction {
    pub token: String,
    pub timestamp: OffsetDateTime,
}
