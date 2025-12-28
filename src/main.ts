import { invoke } from "@tauri-apps/api/core";

let isBackgroundMode = false;

async function toggleCameraMode() {
  console.log("Button clicked!");

  try {
    console.log("Invoking toggle_camera_mode...");
    // Call the Tauri command to toggle the camera mode
    isBackgroundMode = await invoke<boolean>("toggle_camera_mode");
    console.log("Invoke completed, new mode:", isBackgroundMode);

    // Update button text
    const button = document.getElementById("toggle-mode");
    if (button) {
      button.textContent = isBackgroundMode
        ? "Switch to Thumbnail Mode"
        : "Switch to Background Mode";
    }

    // Toggle body transparency class for background mode
    document.body.classList.toggle("transparent", isBackgroundMode);

    console.log(
      `Camera mode: ${isBackgroundMode ? "Background" : "Thumbnail"}`,
    );
  } catch (error) {
    console.error("Failed to toggle camera mode:", error);
  }
}

async function initializeMode() {
  try {
    // Get the current mode on startup
    isBackgroundMode = await invoke<boolean>("get_camera_mode");

    const button = document.getElementById("toggle-mode");
    if (button) {
      button.textContent = isBackgroundMode
        ? "Switch to Thumbnail Mode"
        : "Switch to Background Mode";
    }

    document.body.classList.toggle("transparent", isBackgroundMode);
  } catch (error) {
    console.error("Failed to get camera mode:", error);
  }
}

window.addEventListener("DOMContentLoaded", () => {
  // Initialize the current mode
  initializeMode();

  // Set up the toggle button
  const toggleButton = document.getElementById("toggle-mode");
  if (toggleButton) {
    toggleButton.addEventListener("click", toggleCameraMode);
    console.log("Toggle button event listener attached");
  }
});
