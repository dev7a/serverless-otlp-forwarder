use anyhow::Result;
use std::path::Path;
#[cfg(feature = "screenshots")]
pub async fn take_chart_screenshot(
    html_path: &Path,
    screenshot_path: &Path,
    theme: &str,
) -> Result<()> {
    use headless_chrome::{Browser, LaunchOptions};
    // NOTE: This import may appear unresolved in IDEs like Rust Analyzer because
    // headless_chrome generates the protocol bindings at build time using a build script.
    // The type will be available when building with cargo, so this is safe to use.
    use headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption;

    let browser = Browser::new(
        LaunchOptions::default_builder()
            .window_size(Some((1280, 1400)))
            .enable_logging(true) // Enable browser console logging
            .build()
            .unwrap(),
    )?;
    let tab = browser.new_tab()?;

    // Convert to absolute path and create proper file URL
    let absolute_path = html_path.canonicalize()?;
    let url = format!("file://{}", absolute_path.display());

    // Navigate to the page
    tab.navigate_to(&url)?;
    // Replace wait_until_navigated with a fixed delay for local files
    // std::thread::sleep(std::time::Duration::from_secs(2));
    // tab.wait_until_navigated()?;
    // Wait for prepareScreenshot to be defined (max 5s)
    let mut waited = 0;
    let max_wait = 5000;
    let step = 100;
    while waited < max_wait {
        let is_defined = tab.evaluate("typeof prepareScreenshot === 'function'", false)?;
        if is_defined.value == Some(serde_json::Value::Bool(true)) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(step));
        waited += step;
    }
    if waited >= max_wait {
        anyhow::bail!("prepareScreenshot function not found after waiting");
    }

    // Call the prepareScreenshot JS function
    tab.evaluate(&format!("prepareScreenshot('{}')", theme), true)?;

    // Wait for the chart to render
    tab.wait_for_element("#chart")?;
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Capture the entire page
    let png_data = tab.capture_screenshot(CaptureScreenshotFormatOption::Png, None, None, true)?;

    std::fs::write(screenshot_path, png_data)?;
    Ok(())
}

#[cfg(not(feature = "screenshots"))]
pub async fn take_chart_screenshot(
    _html_path: &Path,
    _screenshot_path: &Path,
    _theme: &str,
) -> Result<()> {
    Ok(())
}
