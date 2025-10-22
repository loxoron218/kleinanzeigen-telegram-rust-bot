use std::{
    collections::{HashSet, VecDeque},
    error::Error,
    fs::{read_to_string, write},
    time::Duration,
};

use reqwest::Client;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::{from_slice, from_str, to_string_pretty};
use tokio::{main, time::sleep};

// --- Configuration ---
// IMPORTANT: Replace these with your actual token and chat ID
const TELEGRAM_BOT_TOKEN: &str = "YOUR_TELEGRAM_BOT_TOKEN";
const TELEGRAM_CHAT_ID: &str = "YOUR_GROUP_CHAT_ID";

// The URL is now split to allow inserting the page number
const KLEINANZEIGEN_BASE_URL: &str = "https://www.kleinanzeigen.de/s-zu-verschenken-tauschen";
const KLEINANZEIGEN_URL_SUFFIX: &str = "/04105/c272l4257r10";
const SEEN_ADS_FILE: &str = "seen_ads.json";
const MAX_SEEN_ADS: usize = 1000;
const FIRST_RUN_LIMIT: usize = 25;

/// Represents a single advertisement listing from Kleinanzeigen.
///
/// This struct holds the essential information scraped from the website for each ad.
#[derive(Debug, Serialize, Deserialize)]
struct Ad {
    /// The unique identifier for the ad (e.g., "3170997111").
    id: String,
    /// The title of the ad listing.
    title: String,
    /// The full URL to the ad's page.
    link: String,
    /// The URL of the ad's main image, if available.
    image_url: Option<String>,
}

/// Represents a Telegram API error response.
#[derive(Debug, Deserialize)]
struct TelegramError {
    /// The error code.
    error_code: Option<i32>,
    /// Additional parameters for the error.
    parameters: Option<TelegramErrorParameters>,
}

/// Additional parameters for Telegram API errors.
#[derive(Debug, Deserialize)]
struct TelegramErrorParameters {
    /// Time to wait before retrying (for rate limiting).
    retry_after: Option<i64>,
}

// --- Functions ---
/// Loads the queue of already-seen ad IDs from a JSON file.
///
/// If the file does not exist or contains invalid data, it returns an empty queue.
/// A VecDeque is used to efficiently remove old items from the front.
fn load_seen_ads() -> VecDeque<String> {
    match read_to_string(SEEN_ADS_FILE) {
        Ok(content) => match from_str(&content) {
            Ok(queue) => queue,
            Err(e) => {
                eprintln!("Fehler beim Parsen der Datei {}: {}", SEEN_ADS_FILE, e);
                VecDeque::new()
            }
        },
        Err(e) => {
            eprintln!("Fehler beim Lesen der Datei {}: {}", SEEN_ADS_FILE, e);
            VecDeque::new()
        }
    }
}

/// Saves the provided queue of seen ad IDs to a JSON file.
///
/// The data is pretty-printed for human readability.
fn save_seen_ads(ad_ids: &VecDeque<String>) -> Result<(), Box<dyn Error>> {
    let content = to_string_pretty(ad_ids)?;
    write(SEEN_ADS_FILE, content)?;
    Ok(())
}

/// Scrapes a specific Kleinanzeigen page for free listings.
///
/// # Arguments
/// * `client` - The `reqwest::Client` to use for the HTTP request.
/// * `url` - The exact URL of the Kleinanzeigen page to scrape.
///
/// # Returns
/// A `Vec<Ad>` containing all ads found on the page, or an error if the request fails.
async fn scrape_kleinanzeigen_page(client: &Client, url: &str) -> Result<Vec<Ad>, Box<dyn Error>> {
    println!("Scrape URL: {}", url);
    let response = client.get(url).send().await?.text().await?;
    let document = Html::parse_document(&response);

    // Define CSS selectors to find the necessary elements on the page.
    let ad_selector = Selector::parse("article.aditem").unwrap();
    let title_link_selector = Selector::parse("a.ellipsis").unwrap();
    let image_selector = Selector::parse(".aditem-image img").unwrap();
    let mut listings = Vec::new();

    // Iterate over each ad container found on the page.
    for article in document.select(&ad_selector) {
        // Extract the unique ad ID from the 'data-adid' attribute.
        if let Some(ad_id) = article.value().attr("data-adid") {
            // Find the primary link within the ad, which contains the title.
            if let Some(link_element) = article.select(&title_link_selector).next() {
                if let Some(href) = link_element.value().attr("href") {
                    // We only care about actual ad links, not other miscellaneous links.
                    if href.starts_with("/s-anzeige/") {
                        let title = link_element.text().collect::<String>().trim().to_string();
                        let full_link = format!("https://www.kleinanzeigen.de{}", href);

                        // --- IMPROVED IMAGE QUALITY FIX ---
                        // Prioritize `srcset` for the best quality image, then fall back to `src`.
                        let image_url = article
                            .select(&image_selector)
                            .next()
                            .and_then(|img| {
                                // `srcset` provides multiple image sizes. We take the last one, which is usually the highest resolution.
                                if let Some(srcset) = img.value().attr("srcset") {
                                    srcset
                                        .split(',')
                                        .last()
                                        .and_then(|s| s.split_whitespace().next())
                                        .map(String::from)
                                } else {
                                    // Fallback to the `src` attribute if `srcset` is not available.
                                    img.value().attr("src").map(String::from)
                                }
                            })
                            .map(|src| {
                                // Get the base URL by splitting at the '?' and taking the first part.
                                if let Some(base_url) = src.split('?').next() {
                                    // Append the high-resolution rule.
                                    format!("{}?rule=$_59.AUTO", base_url)
                                } else {
                                    // If splitting fails for some reason, return the original src.
                                    src
                                }
                            });
                        listings.push(Ad {
                            id: ad_id.to_string(),
                            title,
                            link: full_link,
                            image_url,
                        });
                    }
                }
            }
        }
    }

    // Return the vector of scraped ads
    Ok(listings)
}

/// Sends a photo with a caption to the configured Telegram group.
///
/// # Arguments
/// * `client` - The `reqwest::Client` to use for the API call.
/// * `photo_url` - The URL of the image to send.
/// * `caption` - The HTML-formatted caption for the photo.
/// Sends a photo with a caption to the configured Telegram group.
///
/// # Arguments
/// * `client` - The `reqwest::Client` to use for the API call.
/// * `photo_url` - The URL of the image to send.
/// * `caption` - The HTML-formatted caption for the photo.
async fn send_photo_message(
    client: &Client,
    photo_url: &str,
    caption: &str,
) -> Result<Option<i64>, Box<dyn Error>> {
    let url = format!(
        "https://api.telegram.org/bot{}/sendPhoto",
        TELEGRAM_BOT_TOKEN
    );

    // Use form data to match what curl is doing, with HTML formatting
    let params = [
        ("chat_id", TELEGRAM_CHAT_ID),
        ("photo", photo_url),
        ("caption", caption),
        ("parse_mode", "HTML"),
    ];

    // Send the POST request to the Telegram API with the photo and caption
    let response = client.post(&url).form(&params).send().await?;

    // Check if the response is successful
    if response.status().is_success() {
        println!("Fotonachricht erfolgreich gesendet.");
        return Ok(None);
    }

    // Handle error response
    let status = response.status();
    let error_bytes = response.bytes().await?;

    // Try to parse the error response as JSON
    if let Ok(telegram_error) = from_slice::<TelegramError>(&error_bytes) {
        if telegram_error.error_code == Some(429) {
            // Rate limiting error
            if let Some(params) = telegram_error.parameters {
                if let Some(retry_after) = params.retry_after {
                    return Ok(Some(retry_after));
                }
            }

            // Default retry after 30 seconds if not specified
            return Ok(Some(30));
        }
    }

    // Construct and return a detailed error message with status code and response body
    let error_body = String::from_utf8_lossy(&error_bytes);
    let error_message = format!("Telegram API Fehler: {} - {}", status, error_body);
    Err(error_message.into())
}

/// Sends a text-only message to the configured Telegram group.
///
/// # Arguments
/// * `client` - The `reqwest::Client` to use for the API call.
/// * `message` - The HTML-formatted message string to send.
async fn send_text_message(client: &Client, message: &str) -> Result<Option<i64>, Box<dyn Error>> {
    let url = format!(
        "https://api.telegram.org/bot{}/sendMessage",
        TELEGRAM_BOT_TOKEN
    );

    // Use form data to match what curl is doing, with HTML formatting
    let params = [
        ("chat_id", TELEGRAM_CHAT_ID),
        ("text", message),
        ("parse_mode", "HTML"),
    ];

    // Send the POST request to the Telegram API with the text message
    let response = client.post(&url).form(&params).send().await?;

    // Check if the response is successful
    if response.status().is_success() {
        println!("Textnachricht erfolgreich gesendet.");
        return Ok(None);
    }

    // Handle error response
    let status = response.status();
    let error_bytes = response.bytes().await?;

    // Try to parse the error response as JSON
    if let Ok(telegram_error) = from_slice::<TelegramError>(&error_bytes) {
        if telegram_error.error_code == Some(429) {
            // Rate limiting error
            if let Some(params) = telegram_error.parameters {
                if let Some(retry_after) = params.retry_after {
                    return Ok(Some(retry_after));
                }
            }

            // Default retry after 30 seconds if not specified
            return Ok(Some(30));
        }
    }

    // Construct and return a detailed error message with status code and response body
    let error_body = String::from_utf8_lossy(&error_bytes);
    let error_message = format!("Telegram API Fehler: {} - {}", status, error_body);
    Err(error_message.into())
}

// --- Main Program ---
#[main]
async fn main() -> Result<(), Box<dyn Error>> {
    // A simple guard to prevent running with placeholder credentials.
    if TELEGRAM_BOT_TOKEN == "YOUR_TELEGRAM_BOT_TOKEN" || TELEGRAM_CHAT_ID == "YOUR_GROUP_CHAT_ID" {
        eprintln!(
            "FEHLER: Bitte ersetze die Platzhalter für TELEGRAM_BOT_TOKEN und TELEGRAM_CHAT_ID im Skript."
        );
        return Ok(());
    }

    // Initialize an HTTP client with a browser-like User-Agent to avoid being blocked.
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
        .build()?;

    // Load the IDs of ads we've already notified about.
    let mut seen_ads_queue = load_seen_ads();
    let is_first_run = seen_ads_queue.is_empty();
    println!(
        "{} bereits gesehene Anzeigen geladen.",
        seen_ads_queue.len()
    );

    // Debug print the first few seen ad IDs
    let first_few: Vec<&String> = seen_ads_queue.iter().take(5).collect();
    println!("Erste gesehene IDs: {:?}", first_few);

    // For fast lookups, create a HashSet from the queue.
    let seen_ads_set: HashSet<_> = seen_ads_queue.iter().cloned().collect();
    let mut new_ads_found_total = 0;

    // Track how many ads we've sent on first run
    let mut first_run_sent_count = 0;

    // A safety limit to prevent excessive requests.
    const MAX_PAGES_TO_SCAN: u32 = 10;

    // --- HYBRID LOGIC IMPLEMENTATION ---
    // 1. Collect all ads from pages first before processing
    let mut all_ads: Vec<Ad> = Vec::new();
    let mut stop_paging = false;

    // Loop through the pages of the search results.
    for page in 1..=MAX_PAGES_TO_SCAN {
        let current_url = if page == 1 {
            // The first page has a slightly different URL format.
            format!("{}{}", KLEINANZEIGEN_BASE_URL, KLEINANZEIGEN_URL_SUFFIX)
        } else {
            format!(
                "{}/seite:{}{}",
                KLEINANZEIGEN_BASE_URL, page, KLEINANZEIGEN_URL_SUFFIX
            )
        };

        // Scrape all ads from the current page.
        let current_ads = scrape_kleinanzeigen_page(&client, &current_url).await?;

        // If a page has no ads, we've reached the end of the results.
        if current_ads.is_empty() {
            println!(
                "Keine Anzeigen auf Seite {} gefunden. Suche wird beendet.",
                page
            );
            break;
        }

        // Check if any ads on this page were already seen
        if current_ads.iter().any(|ad| seen_ads_set.contains(&ad.id)) {
            // Set flag to stop after finishing this page
            stop_paging = true;
        }

        // Add all ads from this page to our master list
        all_ads.extend(current_ads);

        // If we found any old ads on this page, we can stop crawling further pages.
        if stop_paging {
            println!(
                "Bereits gesehene Anzeige auf Seite {} gefunden. Scan wird nach dieser Seite beendet.",
                page
            );
            break;
        }

        // Be polite and wait a moment before scraping the next page.
        // Wait for 1 second before scraping the next page to be respectful to the server
        sleep(Duration::from_secs(1)).await;
    }

    // 2. Process all collected ads afterward, sending notifications only for new ones
    for ad in all_ads {
        // For first run, limit the number of ads sent
        if is_first_run && first_run_sent_count >= FIRST_RUN_LIMIT {
            break;
        }

        if !seen_ads_set.contains(&ad.id) {
            // This is a new ad.
            new_ads_found_total += 1;
            println!("Neue Anzeige gefunden: {}", ad.title);
            let caption = format!(
                "<b>Neuer kostenloser Artikel gefunden!</b>\n<b>Titel:</b> {}\n<a href='{}'>Anzeige ansehen</a>",
                ad.title, ad.link
            );

            // If the ad has an image, send a photo message. Otherwise, send a text message.
            let mut send_success = false;
            if let Some(image_url) = &ad.image_url {
                match send_photo_message(&client, image_url, &caption).await {
                    Ok(None) => {
                        // Success
                        send_success = true;
                    }
                    Ok(Some(retry_after)) => {
                        // Rate limiting, wait and retry
                        eprintln!(
                            "Rate limiting erkannt. Warte {} Sekunden vor erneutem Versuch.",
                            retry_after
                        );

                        // Wait for the specified duration before retrying
                        sleep(Duration::from_secs(retry_after as u64)).await;

                        // Retry once
                        match send_photo_message(&client, image_url, &caption).await {
                            Ok(None) => {
                                // Success on retry
                                send_success = true;
                            }
                            Ok(Some(retry_after)) => {
                                eprintln!("Erneute Rate Limiting. Warte {} Sekunden.", retry_after);

                                // Wait for the specified duration before final retry
                                sleep(Duration::from_secs(retry_after as u64)).await;

                                // Final retry
                                if send_photo_message(&client, image_url, &caption)
                                    .await
                                    .is_ok()
                                {
                                    send_success = true;
                                }
                            }
                            Err(e) => {
                                eprintln!("Fehler beim erneuten Senden der Fotonachricht: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "Fehler beim Senden der Fotonachricht: {}. Fallback auf Textnachricht.",
                            e
                        );

                        // If sending the photo fails, try sending a text message instead.
                        match send_text_message(&client, &caption).await {
                            Ok(None) => {
                                // Success
                                send_success = true;
                            }
                            Ok(Some(retry_after)) => {
                                // Rate limiting, wait and retry
                                eprintln!(
                                    "Rate limiting erkannt. Warte {} Sekunden vor erneutem Versuch der Textnachricht.",
                                    retry_after
                                );
                                sleep(Duration::from_secs(retry_after as u64)).await;

                                // Retry once
                                if send_text_message(&client, &caption).await.is_ok() {
                                    send_success = true;
                                }
                            }
                            Err(e_text) => {
                                eprintln!("Fehler beim Senden der Textnachricht: {}", e_text);
                            }
                        }
                    }
                }
            } else {
                match send_text_message(&client, &caption).await {
                    Ok(None) => {
                        // Success
                        send_success = true;
                    }
                    Ok(Some(retry_after)) => {
                        // Rate limiting, wait and retry
                        eprintln!(
                            "Rate limiting erkannt. Warte {} Sekunden vor erneutem Versuch der Textnachricht.",
                            retry_after
                        );

                        // Wait for the specified duration before retrying
                        sleep(Duration::from_secs(retry_after as u64)).await;

                        // Retry once
                        if send_text_message(&client, &caption).await.is_ok() {
                            send_success = true;
                        }
                    }
                    Err(e) => {
                        eprintln!("Fehler beim Senden der Textnachricht: {}", e);
                    }
                }
            }

            // Only add the ad to seen_ads_queue if sending was successful
            if send_success {
                // Add the new ad's ID to our queue to preserve order.
                seen_ads_queue.push_back(ad.id.clone());

                // Increment counter for first run
                if is_first_run {
                    first_run_sent_count += 1;
                }
            } else {
                eprintln!(
                    "Nachricht für Anzeige '{}' wurde nicht erfolgreich gesendet und wird erneut versucht beim nächsten Durchlauf.",
                    ad.title
                );
            }

            // Pause briefly to avoid hitting Telegram's rate limits.
            // Wait for 2 seconds between sending messages to avoid rate limiting
            sleep(Duration::from_secs(2)).await;
        }
    }

    // After scanning, check if we found any new ads.
    if new_ads_found_total > 0 {
        println!(
            "Verarbeitung abgeschlossen. Insgesamt {} neue Anzeige(n) gefunden.",
            new_ads_found_total
        );

        // --- PRUNING LOGIC ---
        // If the queue is now larger than the limit, remove the oldest items from the front.
        while seen_ads_queue.len() > MAX_SEEN_ADS {
            seen_ads_queue.pop_front();
        }
        println!(
            "Die Liste der gesehenen Anzeigen wurde auf {} Einträge gekürzt.",
            seen_ads_queue.len()
        );

        // Save the updated list of seen ads to the file for the next run.
        if let Err(e) = save_seen_ads(&seen_ads_queue) {
            eprintln!(
                "Fehler beim Speichern der Datei mit gesehenen Anzeigen: {}",
                e
            );
        }
    } else {
        println!("Keine neuen Anzeigen auf den gescannten Seiten gefunden.");
    }

    // Print final message and return success
    println!("Skript beendet.");
    Ok(())
}
