use std::{
    collections::HashMap,
    fmt::Debug,
    io::{BufRead, Write},
    sync::Arc,
};

use anyhow::Result;
use chrono::{DateTime, Utc};
use reqwest::header::{self, HeaderMap, HeaderValue};
use tokio::{self, fs};
use ua_generator::ua::spoof_ua;

use crate::macros::concat_sstr;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct CourseGeneralInfo {
    pub id: String,
    pub name: String,
    pub code: String,
    pub credits: i32,
    pub description: String,
    pub department: String,
    pub preadvised: bool,
}

#[derive(Debug, serde::Deserialize)]
pub struct MyCourseInfo {
    pub id: String,
    pub course_id: String,
    pub course_code: String,
    pub course_name: String,
    pub credits: i32,
    pub department: String,
    pub trimester_id: String,
    pub trimester_name: String,
    pub status: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct MyCoursesAndInfo {
    pub user_id: String,
    pub user_info: serde_json::Value,
    pub courses: Vec<MyCourseInfo>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CourseData {
    pub user_id: String,
    pub course_info: Option<CourseInfo>,
    pub is_preadvised: bool,
    pub selection_allowed: bool,
    pub preadvice_course: Vec<PreadviceCourse>,
    pub selection_message: String,
    pub sections: Option<Vec<Section>>,
    // Not sure what are these...
    pub mapped_sections: Option<Vec<serde_json::Value>>,
    pub user_enrollment: Option<serde_json::Value>,
    pub cache_info: CacheInfo,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CourseInfo {
    pub id: String,
    pub name: String,
    pub code: String,
    pub credits: i32,
    pub description: String,
    pub department: String,
    pub preadvised: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Section {
    pub id: String,
    pub section_name: String,
    pub total_seats: i32,
    pub available_seats: i32,
    pub waitlist_count: i32,
    pub faculty_name: String,
    pub faculty_email: String,
    pub faculty_code: String,
    pub room_details: String,
    pub schedule: HashMap<String, String>,
    pub is_active: bool,
    pub can_enroll: bool,
    pub enrollment_status: String,
    pub quotas: Vec<Quota>,
    pub is_mapped: bool,
    pub original_course: Option<CourseInfo>,
    /* REDACTED FIELDS
    pub conflict_with: String,
    pub have_conflict: bool,
     */
    pub can_not_remove: bool,
    pub already_taken: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Quota {
    pub id: String,
    pub department_id: String,
    pub department_name: String,
    pub quota: i32,
    pub taken: i32,
    pub available: i32,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CacheInfo {
    pub is_cached: bool,
    pub cached_at: String,
    pub expires_at: String,
}

#[derive(Debug, serde::Serialize)]
pub struct LoginRequest {
    pub user_id: String,
    pub password: String,
    pub logout_other_sessions: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct SectionActionRequest {
    pub section_id: u64,
    pub action: String,
    pub parent_course_code: String,
}

#[derive(serde::Deserialize, Debug)]
struct Response<T: Sized + Debug> {
    status: String,
    data: Option<T>,
    message: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct Login {
    access_token: String,
    refresh_token: String,
    access_token_expires_at: DateTime<Utc>,
    refresh_token_expires_at: DateTime<Utc>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PreadviceCourse {
    pub running_session: String,
    pub course_code: String,
    pub course_name: String,
    pub formal_code: String,
    pub ucam_ref: u64,
    pub credits: usize,
    pub last_synced_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PreadviceCourses {
    pub user_id: String,
    pub running_session: String,
    pub courses: Vec<PreadviceCourse>,
    pub total_courses: usize,
    pub total_credits: usize,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CourseSection {
    pub section_id: u64,
    pub section_name: String,
    pub total_seats: usize,
    pub seats_taken: usize,
    pub is_enrolled: bool,
    pub faculty_name: String,
    pub faculty_email: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CourseSections {
    pub course_code: String,
    pub course_name: String,
    pub sections: Vec<CourseSection>,
    pub selection_open: bool,
    pub running_session: String,
    pub credits: usize,
    pub section_selection_start_time: DateTime<Utc>,
    pub section_selection_end_time: DateTime<Utc>,
}

pub const ORIGIN: &str = "https://m5p10igya2.execute-api.ap-southeast-1.amazonaws.com";
pub const LOGIN_PATH: &str = "/v3/auth/login";
pub const PREADVICE_COURSES_PATH: &str = "/v3/users/me/preadvice-courses";
pub const SECTIONS_PATH: &str = "/v3/courses/sections";

pub async fn login_client(login_req: &LoginRequest) -> Result<reqwest::Client> {
    const URI: &str = concat_sstr!(ORIGIN, LOGIN_PATH);

    let ua = spoof_ua();
    let cookie_jar = Arc::new(reqwest::cookie::Jar::default());
    let client = reqwest::Client::builder()
        .user_agent(ua)
        //.cookie_provider(cookie_jar.clone())
        .build()?;

    let result = client.post(URI).json(&login_req).send().await?;

    let response = result.json::<Response<Login>>().await?;
    if response.status != "success" {
        anyhow::bail!(
            "Login failed: {:?}",
            response.message.unwrap_or(response.status)
        );
    }
    let response = response
        .data
        .ok_or(anyhow::anyhow!("Data parsing failed!"))?;
    let mut headers = HeaderMap::new();
    headers.append(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", response.access_token))?,
    );
    headers.append(
        header::ORIGIN,
        HeaderValue::from_static("https://ucamcloud.uiu.ac.bd"),
    );
    headers.append(
        header::REFERER,
        HeaderValue::from_static("https://ucamcloud.uiu.ac.bd/"),
    );
    headers.append(header::ACCEPT, HeaderValue::from_static("*/*"));

    return Ok(reqwest::Client::builder()
        .user_agent(ua)
        //.cookie_provider(cookie_jar)
        .default_headers(headers)
        .build()?);
}

pub async fn fetch_all_courses(client: &reqwest::Client) -> Result<Vec<CourseGeneralInfo>> {
    todo!("Not implemeneted yet!");
    // let result = client.get("https://t8kdcntnt1.execute-api.ap-southeast-1.amazonaws.com/v1/sections/routine/courses/department/endpoint-url-does-matter-at-all-so-whatever").send().await?;
    // let response: Response<Vec<CourseGeneralInfo>> = result.json().await?;
    // if response.status != "success" {
    //     anyhow::bail!(
    //         "Fetch all courses failed: {:?}",
    //         response.message.unwrap_or(response.status)
    //     );
    // }
    // Ok(response
    //     .data
    //     .ok_or(anyhow::anyhow!("Data parsing failed!"))?)
}

pub async fn fetch_preadvised_courses(client: &reqwest::Client) -> Result<PreadviceCourses> {
    const URI: &str = concat_sstr!(ORIGIN, PREADVICE_COURSES_PATH);
    let result = client.get(URI).send().await?;
    let response: Response<PreadviceCourses> = result.json().await?;
    if response.status != "success" {
        anyhow::bail!(
            "Fetch preadvised courses failed: {:?}",
            response.message.unwrap_or(response.status)
        );
    }
    Ok(response
        .data
        .ok_or(anyhow::anyhow!("Data parsing failed!"))?)
}

pub async fn fetch_course_sections(
    client: &reqwest::Client,
    course_id: &str,
    student_id: &str,
) -> Result<CourseSections> {
    //todo!("Not implemeneted yet!");
    const URI: &str = concat_sstr!(ORIGIN, SECTIONS_PATH);
    let result = client
        .get(format!("{URI}/{course_id}?student_id={student_id}"))
        .send()
        .await?;
    let response: Response<CourseSections> = result.json().await?;
    if response.status != "success" {
        anyhow::bail!(
            "Fetch course routine failed: {:?}",
            response.message.unwrap_or(response.status)
        );
    }
    Ok(response
        .data
        .ok_or(anyhow::anyhow!("Data parsing failed!"))?)
}

pub async fn fetch_course_data_as_student(
    client: &reqwest::Client,
    course_id: &str,
) -> Result<CourseData> {
    todo!("Not implemeneted yet!");
    // let result = client.get(format!("https://t8kdcntnt1.execute-api.ap-southeast-1.amazonaws.com/v1/sections/course/{course_id}/student")).send().await?;
    // let response: Response<CourseData> = result.json().await?;
    // if response.status != "success" {
    //     anyhow::bail!(
    //         "Get course info failed: {:?}",
    //         response.message.unwrap_or(response.status)
    //     );
    // }
    // Ok(response
    //     .data
    //     .ok_or(anyhow::anyhow!("Data parsing failed!"))?)
}

pub async fn post_course_action(
    client: &reqwest::Client,
    course_id: &str,
    action: &SectionActionRequest,
) -> Result<()> {
    const URI: &str = concat_sstr!(ORIGIN, SECTIONS_PATH);
    let result = client
        .post(format!("{URI}/{course_id}/select"))
        .json(action)
        .send()
        .await?;
    let response: Response<serde_json::Value> = result.json().await?;
    if response.status != "success" {
        anyhow::bail!(
            "Course section action failed: {:?}",
            response.message.unwrap_or(response.status)
        );
    }
    Ok(())
}
