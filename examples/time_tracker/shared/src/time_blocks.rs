use crate::*;

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "serde")]
pub struct Client {
    pub id: ClientId,
    pub name: String,
    pub time_blocks: Vec<TimeBlock>,
    #[serde(with = "DurationSecondsForSerde")]
    pub tracked: Duration,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "serde")]
pub struct TimeBlock {
    pub id: TimeBlockId,
    pub name: String,
    pub status: TimeBlockStatus,
    #[serde(with = "DurationSecondsForSerde")]
    pub duration: Duration,
    pub invoice: Option<Invoice>,
}

#[derive(Default, Debug, Serialize, Deserialize, Copy, Clone, PartialEq, Eq)]
#[serde(crate = "serde")]
pub enum TimeBlockStatus {
    NonBillable,
    #[default]
    Unpaid,
    Paid,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "serde")]
pub struct Invoice {
    pub id: InvoiceId,
    pub custom_id: String,
    pub url: String,
}
