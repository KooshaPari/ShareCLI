// SPDX-License-Identifier: Apache-2.0 OR MIT
#!/usr/bin/env rust-script

// Test script to reproduce session management issues
use std::process::Command;
use std::thread;
use std::time::Duration;

fn main() {
    println!("Testing KVirtualStage session management issues...\n");
    
    // Test 1: Create a session
    println!("1. Creating session 'test-session'...");
    let output = Command::new("cargo")
        .args(&["run", "--", "session", "create", "--name", "test-session", "--desktop", "kubuntu"])
        .output()
        .expect("Failed to create session");
    
    println!("Create output: {}", String::from_utf8_lossy(&output.stdout));
    if !output.stderr.is_empty() {
        println!("Create stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    // Test 2: List sessions immediately after creation
    println!("\n2. Listing sessions after creation...");
    let output = Command::new("cargo")
        .args(&["run", "--", "session", "list"])
        .output()
        .expect("Failed to list sessions");
    
    println!("List output: {}", String::from_utf8_lossy(&output.stdout));
    if !output.stderr.is_empty() {
        println!("List stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    // Test 3: Try to connect to the session
    println!("\n3. Connecting to session 'test-session'...");
    let output = Command::new("cargo")
        .args(&["run", "--", "session", "connect", "test-session"])
        .output()
        .expect("Failed to connect to session");
    
    println!("Connect output: {}", String::from_utf8_lossy(&output.stdout));
    if !output.stderr.is_empty() {
        println!("Connect stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    // Test 4: Check status
    println!("\n4. Checking system status...");
    let output = Command::new("cargo")
        .args(&["run", "--", "status"])
        .output()
        .expect("Failed to check status");
    
    println!("Status output: {}", String::from_utf8_lossy(&output.stdout));
    if !output.stderr.is_empty() {
        println!("Status stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    // Test 5: Try to stop the session
    println!("\n5. Stopping session 'test-session'...");
    let output = Command::new("cargo")
        .args(&["run", "--", "session", "stop", "test-session"])
        .output()
        .expect("Failed to stop session");
    
    println!("Stop output: {}", String::from_utf8_lossy(&output.stdout));
    if !output.stderr.is_empty() {
        println!("Stop stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    // Test 6: Try to remove the session
    println!("\n6. Removing session 'test-session'...");
    let output = Command::new("cargo")
        .args(&["run", "--", "session", "remove", "test-session"])
        .output()
        .expect("Failed to remove session");
    
    println!("Remove output: {}", String::from_utf8_lossy(&output.stdout));
    if !output.stderr.is_empty() {
        println!("Remove stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    println!("\nTest completed. Issues identified:");
    println!("- Sessions don't persist between CLI commands");
    println!("- Each command creates a new KVirtualStageCore instance");
    println!("- VNC port information is not updated in session info");
    println!("- No persistence layer for session storage");
}