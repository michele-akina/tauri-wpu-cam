import { invoke } from "@tauri-apps/api/core";

let isBackgroundMode = false;

async function toggleCameraMode() {
  console.log("Button clicked!");

  try {
    isBackgroundMode = await invoke<boolean>("toggle_camera_mode");

    const button = document.getElementById("toggle-mode");
    if (button) {
      button.textContent = isBackgroundMode
        ? "Switch to Thumbnail Mode"
        : "Switch to Background Mode";
    }
    document.body.classList.toggle("transparent", isBackgroundMode);
  } catch (error) {
    console.error("Failed to toggle camera mode:", error);
  }
}

async function initializeMode() {
  try {
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
  initializeMode();

  const toggleButton = document.getElementById("toggle-mode");
  if (toggleButton) {
    toggleButton.addEventListener("click", toggleCameraMode);
    console.log("Toggle button event listener attached");
  }
});
