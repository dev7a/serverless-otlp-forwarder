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
            .window_size(Some((1280, 2560))) // Initial size, will be resized based on content
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

    // Wait for page navigation and DOM/scripts to load completely
    // std::thread::sleep(std::time::Duration::from_secs(5));
    // tab.wait_until_navigated()?;
    // Wait for prepareScreenshot function to be available (similar to wait_for_element pattern)
    // Try multiple times with different checks to diagnose the issue
    let mut waited = 0;
    let max_wait = 15000;
    let step = 300;
    while waited < max_wait {
        // Check if function is defined
        let is_defined = tab.evaluate("typeof window.prepareScreenshot === 'function'", false)?;
        if is_defined.value == Some(serde_json::Value::Bool(true)) {
            break;
        }

        std::thread::sleep(std::time::Duration::from_millis(step));
        waited += step;
    }
    if waited >= max_wait {
        anyhow::bail!(
            "prepareScreenshot function not found after waiting {}ms",
            max_wait
        );
    }

    // Call the prepareScreenshot JS function
    tab.evaluate(&format!("window.prepareScreenshot('{}')", theme), true)?;

    // Wait for charts to render and themes to apply
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Get the actual content height and resize viewport if needed
    let content_height_result = tab.evaluate("Math.max(document.body.scrollHeight, document.body.offsetHeight, document.documentElement.clientHeight, document.documentElement.scrollHeight, document.documentElement.offsetHeight)", false)?;
    if let Some(serde_json::Value::Number(height_num)) = content_height_result.value {
        if let Some(content_height) = height_num.as_u64() {
            let required_height = content_height + 100; // Add some padding
            if required_height > 2560 {
                // Create new browser with correct size
                drop(tab);
                drop(browser);

                let browser = Browser::new(
                    LaunchOptions::default_builder()
                        .window_size(Some((1280, required_height as u32)))
                        .enable_logging(true)
                        .build()
                        .unwrap(),
                )?;
                let tab = browser.new_tab()?;

                // Re-navigate and prepare
                tab.navigate_to(&url)?;
                std::thread::sleep(std::time::Duration::from_secs(3));

                // Wait for prepareScreenshot function again
                let mut waited = 0;
                let max_wait = 10000; // Shorter wait since we know it works
                let step = 200;
                while waited < max_wait {
                    let is_defined =
                        tab.evaluate("typeof window.prepareScreenshot === 'function'", false)?;
                    if is_defined.value == Some(serde_json::Value::Bool(true)) {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(step));
                    waited += step;
                }

                // Re-prepare screenshot
                tab.evaluate(&format!("window.prepareScreenshot('{}')", theme), true)?;
                std::thread::sleep(std::time::Duration::from_secs(2));

                // Capture with new size
                let png_data =
                    tab.capture_screenshot(CaptureScreenshotFormatOption::Png, None, None, true)?;
                std::fs::write(screenshot_path, png_data)?;
                return Ok(());
            }
        }
    }

    // Capture the entire page with original size
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
