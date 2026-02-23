#![allow(unused)]

use std::{
    collections::HashMap,
    fmt::Debug,
    hash::Hash,
    io::{BufRead, Write},
};

use anyhow::Result;
use tokio::{self, fs};

use crate::ucam_cloud_api::{CourseSections, LoginRequest, SectionActionRequest};

mod macros;
mod ucam_cloud_api;

async fn check_for_dir_and_prompt_remove(path: &str) -> Result<bool> {
    if fs::try_exists(path).await? {
        print!("\"{path}\" already exists. Remove it?(Y/n) ");
        std::io::stdout().flush()?;

        let mut handle = std::io::stdin().lock();
        let mut buf = String::new();
        handle.read_line(&mut buf)?;

        let input = buf.trim().to_lowercase();
        match input.as_str() {
            "" | "y" | "yes" => {
                fs::remove_dir_all(path).await?;
                println!("Removed existing {path} directory.");
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
}

async fn auto_select_section(
    client: reqwest::Client,
    user_id: String,
    course_code: String,
    preferred_sections: Vec<String>,
) -> Result<()> {
    println!(
        "Started auto section selection for course {}, preferred sections: {:?}",
        course_code, preferred_sections
    );
    loop {
        let course_info =
            ucam_cloud_api::fetch_course_sections(&client, &course_code, &user_id).await?;
        if course_info.sections.is_empty() {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            continue;
        }
        if course_info.sections.iter().any(|s| {
            s.is_enrolled
                && preferred_sections.iter().any(|ps| {
                    ps.to_ascii_lowercase()
                        .contains(&s.section_name.to_ascii_lowercase())
                })
        }) {
            println!(
                "Already enrolled in course {}, skipping...",
                course_info.course_name
            );
            return Ok(());
        }
        let mut section_id = None;
        for preferred in preferred_sections.iter() {
            let preferred_lower = preferred.to_ascii_lowercase();
            if let Some(section) = course_info.sections.iter().find(|s| {
                s.section_name
                    .to_ascii_lowercase()
                    .contains(&preferred_lower)
                    && s.seats_taken < s.total_seats
            }) {
                section_id = Some(section.section_id);
                break;
            }
        }
        let Some(section_id) = section_id else {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            continue;
        };
        let action = SectionActionRequest {
            parent_course_code: course_code.to_string(),
            section_id: section_id,
            action: "select".to_string(),
        };
        let result = ucam_cloud_api::post_course_action(&client, &course_code, &action).await;
        println!(
            "{} - Attempted to select section {}, result: {:?}",
            course_info.course_name, section_id, result
        );
        return Ok(());
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<String>>();
    if args.len() != 3 {
        println!(
            "Usage: {} <student_id> <password> | cargo run -- <student_id> <password>",
            args[0]
        );
        return Ok(());
    }
    let login_req = LoginRequest {
        user_id: args[1].clone(),
        password: args[2].clone(),
        logout_other_sessions: false,
    };
    let preferred_sections = HashMap::from([
        // (
        //     "1312-1-1".to_string(),
        //     vec!["D".to_string(), "Q".to_string()],
        // ),
        (
            "1372-1-1".to_string(),
            //vec!["K".to_string(), "B".to_string()],
            vec!["B".to_string()],
        ),
        // (
        //     "1373-1-1".to_string(),
        //     vec!["K".to_string(), "B".to_string()],
        // ),
        // (
        //     "1393-1-1".to_string(),
        //     vec!["J".to_string(), "H".to_string()],
        // ),
    ]);

    loop {
        let client = ucam_cloud_api::login_client(&login_req).await?;
        println!("Logged in successfully.");

        let preadvised = ucam_cloud_api::fetch_preadvised_courses(&client).await?;
        println!("Preadvised courses count: {}", preadvised.courses.len());

        let mut join_set = tokio::task::JoinSet::new();
        for course in preadvised.courses {
            let preferred_sections = preferred_sections
                .get(&course.course_code)
                .cloned()
                .unwrap_or_default();
            if preferred_sections.is_empty() {
                println!(
                    "No preferred sections specified for course {}, skipping...",
                    course.course_code
                );
                continue;
            }
            join_set.spawn(auto_select_section(
                client.clone(),
                login_req.user_id.clone(),
                course.course_code,
                preferred_sections,
            ));
        }
        let res = join_set.join_all().await;
        let mut restart = false;
        for r in res {
            if let Err(e) = r {
                println!("Error in auto section selection task: {:?}", e);
                restart |= format!("{e}").to_lowercase().contains("invalid token");
            }
        }
        if restart {
            println!("Restarting the process due to invalid token...");
            continue;
        }
        break;
    }

    Ok(())
}

async fn main2() -> Result<()> {
    let args = std::env::args().collect::<Vec<String>>();
    if args.len() != 3 {
        println!(
            "Usage: {} <student_id> <password> | cargo run -- <student_id> <password>",
            args[0]
        );
        return Ok(());
    }
    let login_req = LoginRequest {
        user_id: args[1].clone(),
        password: args[2].clone(),
        logout_other_sessions: false,
    };

    let client = ucam_cloud_api::login_client(&login_req).await?;
    println!("Logged in successfully.");
    let all_courses = ucam_cloud_api::fetch_all_courses(&client).await?;
    print!("Total courses fetched: {}\n", all_courses.len());
    fs::write(
        "all-courses.json",
        serde_json::to_string_pretty(&all_courses)?.as_bytes(),
    )
    .await?;

    const SECTIONS_DIR: &'static str = "sections";
    if !check_for_dir_and_prompt_remove(SECTIONS_DIR).await? {
        println!("Aborting...");
        return Ok(());
    }
    fs::create_dir(SECTIONS_DIR).await?;
    for course in all_courses.iter() {
        let file_path = format!("{}/{}.json", SECTIONS_DIR, course.id);
        let sections_data =
            ucam_cloud_api::fetch_course_sections(&client, &course.id, &login_req.user_id).await?;
        let sections = sections_data.sections;
        let content = serde_json::to_string_pretty(&sections)?;
        fs::write(&file_path, content).await?;
        println!(
            "Wrote sections({}) for course {} to {}",
            sections.len(),
            course.code,
            file_path
        );
    }

    const SECTIONS_STUDENT_VIEW_DIR: &'static str = "sections_student_view";
    if !check_for_dir_and_prompt_remove(SECTIONS_STUDENT_VIEW_DIR).await? {
        println!("Aborting...");
        return Ok(());
    }
    fs::create_dir(SECTIONS_STUDENT_VIEW_DIR).await?;
    for course in all_courses.iter() {
        let file_path = format!("{}/{}.json", SECTIONS_STUDENT_VIEW_DIR, course.id);
        let course_data = ucam_cloud_api::fetch_course_data_as_student(&client, &course.id).await?;
        let content = serde_json::to_string_pretty(&course_data)?;
        fs::write(&file_path, content).await?;
        let sections_count = match &course_data.sections {
            Some(sections) => sections.len(),
            None => 0,
        };
        println!(
            "Wrote Course data for course {} to {}, sections count: {}",
            course.code, file_path, sections_count,
        );
    }
    Ok(())
}
