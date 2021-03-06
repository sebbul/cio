use std::collections::BTreeMap;
use std::str::from_utf8;

use airtable_api::{Airtable, Record};
use chrono::NaiveDate;
use hubcaps::Github;
use serde::{Deserialize, Serialize};

use crate::airtable::{
    airtable_api_key, AIRTABLE_BASE_ID_MISC, AIRTABLE_GRID_VIEW,
    AIRTABLE_JOURNAL_CLUB_MEETINGS_TABLE, AIRTABLE_JOURNAL_CLUB_PAPERS_TABLE,
};
use crate::db::Database;
use crate::models::{
    JournalClubMeeting, JournalClubPaper, NewJournalClubMeeting,
    NewJournalClubPaper,
};
use crate::utils::github_org;

#[derive(Debug, Deserialize, Serialize)]
pub struct Meeting {
    pub title: String,
    pub issue: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub papers: Vec<NewJournalClubPaper>,
    #[serde(
        deserialize_with = "meeting_date_format::deserialize",
        serialize_with = "meeting_date_format::serialize"
    )]
    pub issue_date: NaiveDate,
    #[serde(
        deserialize_with = "meeting_date_format::deserialize",
        serialize_with = "meeting_date_format::serialize"
    )]
    pub meeting_date: NaiveDate,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub coordinator: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub state: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub recording: String,
}

mod meeting_date_format {
    use chrono::NaiveDate;
    use serde::{self, Deserialize, Deserializer, Serializer};

    const FORMAT: &str = "%m/%d/%Y";
    pub const DEFAULT_DATE: &str = "01/01/1969";

    // The signature of a serialize_with function must follow the pattern:
    //
    //    fn serialize<S>(&T, S) -> Result<S::Ok, S::Error>
    //    where
    //        S: Serializer
    //
    // although it may also be generic over the input types T.
    pub fn serialize<S>(
        date: &NaiveDate,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = format!("{}", date.format(FORMAT));
        if s == DEFAULT_DATE {
            s = "".to_string();
        }
        serializer.serialize_str(&s)
    }

    // The signature of a deserialize_with function must follow the pattern:
    //
    //    fn deserialize<'de, D>(D) -> Result<T, D::Error>
    //    where
    //        D: Deserializer<'de>
    //
    // although it may also be generic over the output types T.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<NaiveDate, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut s = String::deserialize(deserializer).unwrap();
        if s.trim().is_empty() {
            s = DEFAULT_DATE.to_string();
        }
        NaiveDate::parse_from_str(&s, FORMAT).map_err(serde::de::Error::custom)
    }
}

impl Meeting {
    pub fn to_model(&self) -> NewJournalClubMeeting {
        let mut papers: Vec<String> = Default::default();
        for p in &self.papers {
            let paper = serde_json::to_string_pretty(&p).unwrap();
            papers.push(paper);
        }

        NewJournalClubMeeting {
            title: self.title.to_string(),
            issue: self.issue.to_string(),
            papers,
            issue_date: self.issue_date,
            meeting_date: self.meeting_date,
            coordinator: self.coordinator.to_string(),
            state: self.state.to_string(),
            recording: self.recording.to_string(),
        }
    }
}

/// Get the journal club meetings from the papers GitHub repo.
pub async fn get_meetings_from_repo(github: &Github) -> Vec<Meeting> {
    // Get the contents of the .helpers/meetings.csv file.
    let meetings_csv_content = github
        .repo(github_org(), "papers")
        .content()
        .file("/.helpers/meetings.json", "master")
        .await
        .expect("failed to get meetings csv content")
        .content;
    let meetings_json_string = from_utf8(&meetings_csv_content).unwrap();

    // Parse the meetings from the json string.
    let meetings: Vec<Meeting> =
        serde_json::from_str(meetings_json_string).unwrap();

    meetings
}

// Sync the journal_club_meetings with our database.
pub async fn refresh_db_journal_club_meetings(github: &Github) {
    let journal_club_meetings = get_meetings_from_repo(github).await;

    // Initialize our database.
    let db = Database::new();

    // Sync journal_club_meetings.
    for journal_club_meeting in journal_club_meetings {
        db.upsert_journal_club_meeting(&journal_club_meeting.to_model());

        // Upsert the papers.
        for mut journal_club_paper in journal_club_meeting.papers {
            journal_club_paper.meeting = journal_club_meeting.issue.to_string();
            db.upsert_journal_club_paper(&journal_club_paper);
        }
    }
}

pub async fn refresh_airtable_journal_club_meetings() {
    // Initialize the Airtable client.
    let airtable = Airtable::new(airtable_api_key(), AIRTABLE_BASE_ID_MISC);

    let records = airtable
        .list_records(
            AIRTABLE_JOURNAL_CLUB_MEETINGS_TABLE,
            AIRTABLE_GRID_VIEW,
            vec![],
        )
        .await
        .unwrap();

    let mut airtable_journal_club_meetings: BTreeMap<
        i32,
        (Record, JournalClubMeeting),
    > = Default::default();
    for record in records {
        let fields: JournalClubMeeting =
            serde_json::from_value(record.fields.clone()).unwrap();

        airtable_journal_club_meetings.insert(fields.id, (record, fields));
    }

    // Initialize our database.
    let db = Database::new();
    let journal_club_meetings = db.get_journal_club_meetings();

    let mut updated: i32 = 0;
    for mut journal_club_meeting in journal_club_meetings {
        // Reset the papers field.
        journal_club_meeting.papers = Default::default();

        // See if we have it in our fields.
        match airtable_journal_club_meetings.get(&journal_club_meeting.id) {
            Some((r, in_airtable_fields)) => {
                let mut record = r.clone();

                // Set the papers fileds.
                journal_club_meeting.papers = in_airtable_fields.papers.clone();

                record.fields = json!(journal_club_meeting);

                airtable
                    .update_records(
                        AIRTABLE_JOURNAL_CLUB_MEETINGS_TABLE,
                        vec![record.clone()],
                    )
                    .await
                    .unwrap();

                updated += 1;
            }
            None => {
                // Create the record.
                journal_club_meeting.push_to_airtable().await;
            }
        }
    }

    println!("updated {} journal_club_meetings", updated);
}

pub async fn refresh_airtable_journal_club_papers() {
    // Initialize the Airtable client.
    let airtable = Airtable::new(airtable_api_key(), AIRTABLE_BASE_ID_MISC);

    let records = airtable
        .list_records(
            AIRTABLE_JOURNAL_CLUB_PAPERS_TABLE,
            AIRTABLE_GRID_VIEW,
            vec![],
        )
        .await
        .unwrap();

    let mut airtable_journal_club_papers: BTreeMap<
        i32,
        (Record, JournalClubPaper),
    > = Default::default();
    for record in records {
        let fields: JournalClubPaper =
            serde_json::from_value(record.fields.clone()).unwrap();

        airtable_journal_club_papers.insert(fields.id, (record, fields));
    }

    let meeting_records = airtable
        .list_records(
            AIRTABLE_JOURNAL_CLUB_MEETINGS_TABLE,
            AIRTABLE_GRID_VIEW,
            vec![],
        )
        .await
        .unwrap();

    let mut airtable_journal_club_meetings: BTreeMap<String, String> =
        Default::default();
    for meeting_record in meeting_records {
        let fields: JournalClubMeeting =
            serde_json::from_value(meeting_record.fields.clone()).unwrap();

        airtable_journal_club_meetings
            .insert(fields.issue, meeting_record.id.unwrap());
    }

    // Initialize our database.
    let db = Database::new();
    let journal_club_papers = db.get_journal_club_papers();

    let mut updated: i32 = 0;
    for mut journal_club_paper in journal_club_papers {
        // Set the link_to_meeting to the right meeting.
        let meeting_record_id = if let Some(m) =
            airtable_journal_club_meetings.get(&journal_club_paper.meeting)
        {
            m.to_string()
        } else {
            "".to_string()
        };
        if !meeting_record_id.is_empty() {
            journal_club_paper.link_to_meeting = vec![meeting_record_id];
        }

        // See if we have it in our fields.
        match airtable_journal_club_papers.get(&journal_club_paper.id) {
            Some((r, _in_airtable_fields)) => {
                let mut record = r.clone();

                record.fields = json!(journal_club_paper);

                airtable
                    .update_records(
                        AIRTABLE_JOURNAL_CLUB_PAPERS_TABLE,
                        vec![record.clone()],
                    )
                    .await
                    .unwrap();

                updated += 1;
            }
            None => {
                // Create the record.
                journal_club_paper.push_to_airtable().await;
            }
        }
    }

    println!("updated {} journal_club_papers", updated);
}

#[cfg(test)]
mod tests {
    use crate::journal_clubs::{
        refresh_airtable_journal_club_meetings,
        refresh_airtable_journal_club_papers, refresh_db_journal_club_meetings,
    };
    use crate::utils::authenticate_github;

    #[tokio::test(threaded_scheduler)]
    async fn test_journal_club_meetings() {
        let github = authenticate_github();
        refresh_db_journal_club_meetings(&github).await;
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_journal_club_meetings_airtable() {
        refresh_airtable_journal_club_meetings().await;
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_journal_club_papers_airtable() {
        refresh_airtable_journal_club_papers().await;
    }
}
