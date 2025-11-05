//! Integration tests for ai-scheduler-rs
//! These tests run with std support

use ai_scheduler_rs::*;

#[test]
fn test_gpu_utilization() {
    set_gpu_utilization(30);
    assert_eq!(get_gpu_utilization(), 30);
    assert!(is_gpu_available());
    
    set_gpu_utilization(70);
    assert_eq!(get_gpu_utilization(), 70);
    assert!(!is_gpu_available());
    
    // Test clamping
    set_gpu_utilization(150);
    assert_eq!(get_gpu_utilization(), 100);
}

#[test]
fn test_task_registration() {
    let count_before = get_ai_task_count();
    register_ai_task(1234).unwrap();
    assert_eq!(get_ai_task_count(), count_before + 1);
    
    unregister_ai_task(1234);
    assert_eq!(get_ai_task_count(), count_before);
}

#[test]
fn test_task_info() {
    let task = AiTaskInfo::new(1234);
    assert_eq!(task.pid, 1234);
    assert_eq!(task.priority, AI_TASK_PRIORITY);
    assert!(task.should_use_gpu());
}

#[test]
fn test_scheduling_decision() {
    let task = AiTaskInfo::new(1234);
    
    // GPU available
    set_gpu_utilization(30);
    assert!(should_schedule_on_gpu(&task));
    
    // GPU busy
    set_gpu_utilization(90);
    assert!(!should_schedule_on_gpu(&task));
}

