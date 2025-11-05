//! Windows.AI.MachineLearning API integration
//! DirectML-based inference for GPU acceleration

use anyhow::{Context, Result};
use tracing::{debug, info, warn};
use windows::AI::MachineLearning::*;
use windows::Storage::*;

/// Machine Learning Runtime
pub struct MlRuntime {
    device: Option<LearningModelDevice>,
    session: Option<LearningModelSession>,
}

impl MlRuntime {
    /// Create new ML runtime
    pub fn new() -> Result<Self> {
        info!("Initializing Windows ML Runtime");
        
        Ok(Self {
            device: None,
            session: None,
        })
    }
    
    /// Initialize with GPU device
    pub async fn initialize_gpu(&mut self) -> Result<()> {
        debug!("Initializing GPU device for Windows ML");
        
        // Create device with DirectX GPU
        let device = LearningModelDevice::CreateFromDirect3D11Device(None)
            .context("Failed to create LearningModelDevice")?;
        
        info!("GPU device created for Windows ML");
        self.device = Some(device);
        
        Ok(())
    }
    
    /// Load ONNX model from file
    pub async fn load_model(&mut self, model_path: &str) -> Result<()> {
        info!("Loading ONNX model: {model_path}");
        
        // Convert to StorageFile
        let file = StorageFile::GetFileFromPathAsync(&model_path.into())
            .context("Failed to open model file")?
            .await
            .context("Failed to await storage file")?;
        
        // Load model
        let model = LearningModel::LoadFromStorageFileAsync(&file)
            .context("Failed to start model loading")?
            .await
            .context("Failed to load model")?;
        
        info!(
            "Model loaded: {} (Author: {}, Version: {})",
            model.Name()?,
            model.Author().unwrap_or_default(),
            model.Version()
        );
        
        // Create session
        if let Some(device) = &self.device {
            let session = LearningModelSession::CreateFromModelOnDevice(&model, device)
                .context("Failed to create session")?;
            
            self.session = Some(session);
            info!("ML session created with GPU device");
        } else {
            let session = LearningModelSession::CreateFromModel(&model)
                .context("Failed to create session")?;
            
            self.session = Some(session);
            warn!("ML session created without GPU (CPU fallback)");
        }
        
        Ok(())
    }
    
    /// Check if GPU is available for ML
    pub fn is_gpu_available(&self) -> bool {
        self.device.is_some()
    }
    
    /// Get device information
    pub fn get_device_info(&self) -> Result<DeviceInfo> {
        if let Some(device) = &self.device {
            // Get device kind
            let kind = device.AdapterDeviceKind()?;
            
            Ok(DeviceInfo {
                kind: format!("{kind:?}"),
                is_gpu: true,
            })
        } else {
            Ok(DeviceInfo {
                kind: "CPU".to_string(),
                is_gpu: false,
            })
        }
    }
}

/// Device information
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub kind: String,
    pub is_gpu: bool,
}

