use moonlight::*;

pub mod clients_and_projects;
pub mod time_blocks;
pub mod time_tracker;

pub type ClientId = EntityId;
pub type ProjectId = EntityId;
pub type TimeBlockId = EntityId;
pub type InvoiceId = EntityId;
pub type TimeEntryId = EntityId;
pub type UserId = EntityId;

#[derive(Serialize, Deserialize)]
#[serde(remote = "Duration")]
#[serde(crate = "serde")]
struct DurationSecondsForSerde {
    #[serde(getter = "Duration::num_seconds")]
    seconds: i64,
}

impl From<DurationSecondsForSerde> for Duration {
    fn from(for_serde: DurationSecondsForSerde) -> Duration {
        Duration::seconds(for_serde.seconds)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "serde")]
pub struct User {
    pub id: UserId,
    pub name: String,
    pub auth_token: AuthToken,
}

// ------ UpMsg ------

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "serde")]
pub enum UpMsg {
    // ------ Auth ------
    Login {
        name: String,
        password: String,
    },
    Logout,
    // ------ Page data ------
    GetClientsAndProjectsClients,
    GetTimeBlocksClients,
    GetTimeTrackerClients,
    // ------ Client ------
    AddClient(ClientId),
    RemoveClient(ClientId),
    RenameClient(ClientId, String),
    // ------ Project ------
    AddProject(ClientId, ProjectId),
    RemoveProject(ProjectId),
    RenameProject(ProjectId, String),
    // ------ TimeBlock ------
    AddTimeBlock(
        ClientId,
        TimeBlockId,
        #[serde(with = "DurationSecondsForSerde")] Duration,
    ),
    RemoveTimeBlock(TimeBlockId),
    RenameTimeBlock(TimeBlockId, String),
    SetTimeBlockStatus(TimeBlockId, time_blocks::TimeBlockStatus),
    SetTimeBlockDuration(
        TimeBlockId,
        #[serde(with = "DurationSecondsForSerde")] Duration,
    ),
    // ------ Invoice ------
    AddInvoice(TimeBlockId, InvoiceId),
    RemoveInvoice(InvoiceId),
    SetInvoiceCustomId(InvoiceId, String),
    SetInvoiceUrl(InvoiceId, String),
    // ------ TimeEntry ------
    AddTimeEntry(ProjectId, time_tracker::TimeEntry),
    RemoveTimeEntry(TimeEntryId),
    RenameTimeEntry(TimeEntryId, String),
    SetTimeEntryStarted(TimeEntryId, DateTime<Local>),
    SetTimeEntryStopped(TimeEntryId, DateTime<Local>),
}

// ------ DownMsg ------

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "serde")]
pub enum DownMsg {
    // ------ Auth ------
    LoginError(String),
    LoggedIn(User),
    LoggedOut,
    AccessDenied,
    // ------ Page data ------
    ClientsAndProjectsClients(Vec<clients_and_projects::Client>),
    TimeBlocksClients(Vec<time_blocks::Client>),
    TimeTrackerClients(Vec<time_tracker::Client>),
    // ------ Client ------
    ClientAdded,
    ClientRemoved,
    ClientRenamed,
    // ------ Project ------
    ProjectAdded,
    ProjectRemoved,
    ProjectRenamed,
    // ------ TimeBlock ------
    TimeBlockAdded,
    TimeBlockRemoved,
    TimeBlockRenamed,
    TimeBlockStatusSet,
    TimeBlockDurationSet,
    // ------ Invoice ------
    InvoiceAdded,
    InvoiceRemoved,
    InvoiceCustomIdSet,
    InvoiceUrlSet,
    // ------ TimeEntry ------
    TimeEntryAdded,
    TimeEntryRemoved,
    TimeEntryRenamed,
    TimeEntryStartedSet,
    TimeEntryStoppedSet,
}
