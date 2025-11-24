use std::ops::{Deref, DerefMut};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{
    RawAudioContent, RawContent, RawEmbeddedResource, RawImageContent, RawResource,
    RawResourceTemplate, RawTextContent, Role,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Annotations {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audience: Option<Vec<Role>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "lastModified")]
    pub last_modified: Option<DateTime<Utc>>,
}

impl Annotations {
    /// Creates a new Annotations instance specifically for resources
    /// optional priority, and a timestamp (defaults to now if None)
    pub fn for_resource(priority: f32, timestamp: DateTime<Utc>) -> Self {
        assert!(
            (0.0..=1.0).contains(&priority),
            "Priority {priority} must be between 0.0 and 1.0"
        );
        Annotations {
            priority: Some(priority),
            last_modified: Some(timestamp),
            audience: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Annotated<T: AnnotateAble> {
    #[serde(flatten)]
    pub raw: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Annotations>,
}

impl<T: AnnotateAble> Deref for Annotated<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

impl<T: AnnotateAble> DerefMut for Annotated<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.raw
    }
}

impl<T: AnnotateAble> Annotated<T> {
    pub fn new(raw: T, annotations: Option<Annotations>) -> Self {
        Self { raw, annotations }
    }
    pub fn remove_annotation(&mut self) -> Option<Annotations> {
        self.annotations.take()
    }
    pub fn audience(&self) -> Option<&Vec<Role>> {
        self.annotations.as_ref().and_then(|a| a.audience.as_ref())
    }
    pub fn priority(&self) -> Option<f32> {
        self.annotations.as_ref().and_then(|a| a.priority)
    }
    pub fn timestamp(&self) -> Option<DateTime<Utc>> {
        self.annotations.as_ref().and_then(|a| a.last_modified)
    }
    pub fn with_audience(self, audience: Vec<Role>) -> Annotated<T>
    where
        Self: Sized,
    {
        if let Some(annotations) = self.annotations {
            Annotated {
                raw: self.raw,
                annotations: Some(Annotations {
                    audience: Some(audience),
                    ..annotations
                }),
            }
        } else {
            Annotated {
                raw: self.raw,
                annotations: Some(Annotations {
                    audience: Some(audience),
                    priority: None,
                    last_modified: None,
                }),
            }
        }
    }
    pub fn with_priority(self, priority: f32) -> Annotated<T>
    where
        Self: Sized,
    {
        if let Some(annotations) = self.annotations {
            Annotated {
                raw: self.raw,
                annotations: Some(Annotations {
                    priority: Some(priority),
                    ..annotations
                }),
            }
        } else {
            Annotated {
                raw: self.raw,
                annotations: Some(Annotations {
                    priority: Some(priority),
                    last_modified: None,
                    audience: None,
                }),
            }
        }
    }
    pub fn with_timestamp(self, timestamp: DateTime<Utc>) -> Annotated<T>
    where
        Self: Sized,
    {
        if let Some(annotations) = self.annotations {
            Annotated {
                raw: self.raw,
                annotations: Some(Annotations {
                    last_modified: Some(timestamp),
                    ..annotations
                }),
            }
        } else {
            Annotated {
                raw: self.raw,
                annotations: Some(Annotations {
                    last_modified: Some(timestamp),
                    priority: None,
                    audience: None,
                }),
            }
        }
    }
    pub fn with_timestamp_now(self) -> Annotated<T>
    where
        Self: Sized,
    {
        self.with_timestamp(Utc::now())
    }
}

mod sealed {
    pub trait Sealed {}
}
macro_rules! annotate {
    ($T: ident) => {
        impl sealed::Sealed for $T {}
        impl AnnotateAble for $T {}
    };
}

annotate!(RawContent);
annotate!(RawTextContent);
annotate!(RawImageContent);
annotate!(RawAudioContent);
annotate!(RawEmbeddedResource);
annotate!(RawResource);
annotate!(RawResourceTemplate);

pub trait AnnotateAble: sealed::Sealed {
    fn optional_annotate(self, annotations: Option<Annotations>) -> Annotated<Self>
    where
        Self: Sized,
    {
        Annotated::new(self, annotations)
    }
    fn annotate(self, annotations: Annotations) -> Annotated<Self>
    where
        Self: Sized,
    {
        Annotated::new(self, Some(annotations))
    }
    fn no_annotation(self) -> Annotated<Self>
    where
        Self: Sized,
    {
        Annotated::new(self, None)
    }
    fn with_audience(self, audience: Vec<Role>) -> Annotated<Self>
    where
        Self: Sized,
    {
        self.annotate(Annotations {
            audience: Some(audience),
            ..Default::default()
        })
    }
    fn with_priority(self, priority: f32) -> Annotated<Self>
    where
        Self: Sized,
    {
        self.annotate(Annotations {
            priority: Some(priority),
            ..Default::default()
        })
    }
    fn with_timestamp(self, timestamp: DateTime<Utc>) -> Annotated<Self>
    where
        Self: Sized,
    {
        self.annotate(Annotations {
            last_modified: Some(timestamp),
            ..Default::default()
        })
    }
    fn with_timestamp_now(self) -> Annotated<Self>
    where
        Self: Sized,
    {
        self.with_timestamp(Utc::now())
    }
}
