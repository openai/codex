// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! This module contains types required to export ICU4X data via the `icu_provider_export` crate.
//! End users should not need to consume anything in this module.
//!
//! This module is enabled with the `export` Cargo feature.

mod payload;

#[doc(hidden)] // macro
pub use payload::ExportBox;
pub use payload::ExportMarker;

use crate::prelude::*;
use alloc::collections::BTreeSet;

/// An object capable of exporting data payloads in some form.
pub trait DataExporter: Sync {
    /// Save a `payload` corresponding to the given marker and locale.
    ///
    /// Takes non-mut self as it can be called concurrently.
    fn put_payload(
        &self,
        marker: DataMarkerInfo,
        id: DataIdentifierBorrowed,
        payload: &DataPayload<ExportMarker>,
    ) -> Result<(), DataError>;

    /// Function called for singleton markers.
    ///
    /// Takes non-mut self as it can be called concurrently.
    fn flush_singleton(
        &self,
        marker: DataMarkerInfo,
        payload: &DataPayload<ExportMarker>,
        metadata: FlushMetadata,
    ) -> Result<(), DataError> {
        self.put_payload(marker, Default::default(), payload)?;
        self.flush(marker, metadata)
    }

    /// Function called after a non-singleton marker has been fully enumerated.
    ///
    /// Takes non-mut self as it can be called concurrently.
    fn flush(&self, _marker: DataMarkerInfo, _metadata: FlushMetadata) -> Result<(), DataError> {
        Ok(())
    }

    /// This function has to be called before the object is dropped (after all
    /// markers have been fully dumped). This conceptually takes ownership, so
    /// clients *may not* interact with this object after close has been called.
    fn close(&mut self) -> Result<ExporterCloseMetadata, DataError> {
        Ok(ExporterCloseMetadata::default())
    }
}

#[derive(Debug, Default)]
#[allow(clippy::exhaustive_structs)] // newtype
/// Contains information about a successful export.
pub struct ExporterCloseMetadata(pub Option<Box<dyn core::any::Any>>);

/// Metadata for [`DataExporter::flush`]
#[non_exhaustive]
#[derive(Debug, Copy, Clone, Default)]
pub struct FlushMetadata {
    /// Whether the data was generated in such a way that a [`DryDataProvider`] implementation
    /// makes sense.
    pub supports_dry_provider: bool,
    /// The checksum to return with this data marker.
    pub checksum: Option<u64>,
}

impl DataExporter for Box<dyn DataExporter> {
    fn put_payload(
        &self,
        marker: DataMarkerInfo,
        id: DataIdentifierBorrowed,
        payload: &DataPayload<ExportMarker>,
    ) -> Result<(), DataError> {
        (**self).put_payload(marker, id, payload)
    }

    fn flush_singleton(
        &self,
        marker: DataMarkerInfo,
        payload: &DataPayload<ExportMarker>,
        metadata: FlushMetadata,
    ) -> Result<(), DataError> {
        (**self).flush_singleton(marker, payload, metadata)
    }

    fn flush(&self, marker: DataMarkerInfo, metadata: FlushMetadata) -> Result<(), DataError> {
        (**self).flush(marker, metadata)
    }

    fn close(&mut self) -> Result<ExporterCloseMetadata, DataError> {
        (**self).close()
    }
}

/// A [`DynamicDataProvider`] that can be used for exporting data.
///
/// Use [`make_exportable_provider`] to implement this.
pub trait ExportableProvider:
    crate::data_provider::IterableDynamicDataProvider<ExportMarker> + Sync
{
    /// Returns the set of supported markers
    fn supported_markers(&self) -> BTreeSet<DataMarkerInfo>;
}

impl ExportableProvider for Box<dyn ExportableProvider> {
    fn supported_markers(&self) -> BTreeSet<DataMarkerInfo> {
        (**self).supported_markers()
    }
}

/// This macro can be used on a data provider to allow it to be exported by `ExportDriver`.
///
/// Data generation 'compiles' data by using this data provider (which usually translates data from
/// different sources and doesn't have to be efficient) to generate data structs, and then writing
/// them to an efficient format like `BlobDataProvider` or `BakedDataProvider`. The requirements
/// for `make_exportable_provider` are:
/// * The data struct has to implement [`serde::Serialize`](::serde::Serialize) and [`databake::Bake`]
/// * The provider needs to implement [`IterableDataProvider`] for all specified [`DataMarker`]s.
///   This allows the generating code to know which [`DataIdentifierCow`]s to export.
#[macro_export]
#[doc(hidden)] // macro
macro_rules! __make_exportable_provider {
    ($provider:ty, [ $($(#[$cfg:meta])? $struct_m:ty),+, ]) => {
        impl $crate::export::ExportableProvider for $provider {
            fn supported_markers(&self) -> alloc::collections::BTreeSet<$crate::DataMarkerInfo> {
                alloc::collections::BTreeSet::from_iter([
                    $(
                        $(#[$cfg])?
                        <$struct_m>::INFO,
                    )+
                ])
            }
        }

        $crate::dynutil::impl_dynamic_data_provider!(
            $provider,
            [ $($(#[$cfg])? $struct_m),+, ],
            $crate::export::ExportMarker
        );

        $crate::dynutil::impl_iterable_dynamic_data_provider!(
            $provider,
            [ $($(#[$cfg])? $struct_m),+, ],
            $crate::export::ExportMarker
        );
    };
}
#[doc(inline)]
pub use __make_exportable_provider as make_exportable_provider;

/// A `DataExporter` that forks to multiple `DataExporter`s.
#[derive(Default)]
pub struct MultiExporter(Vec<Box<dyn DataExporter>>);

impl MultiExporter {
    /// Creates a `MultiExporter` for the given exporters.
    pub const fn new(exporters: Vec<Box<dyn DataExporter>>) -> Self {
        Self(exporters)
    }
}

impl core::fmt::Debug for MultiExporter {
    fn fmt(&self, f: &mut alloc::fmt::Formatter<'_>) -> alloc::fmt::Result {
        f.debug_struct("MultiExporter")
            .field("0", &format!("vec[len = {}]", self.0.len()))
            .finish()
    }
}

impl DataExporter for MultiExporter {
    fn put_payload(
        &self,
        marker: DataMarkerInfo,
        id: DataIdentifierBorrowed,
        payload: &DataPayload<ExportMarker>,
    ) -> Result<(), DataError> {
        self.0
            .iter()
            .try_for_each(|e| e.put_payload(marker, id, payload))
    }

    fn flush_singleton(
        &self,
        marker: DataMarkerInfo,
        payload: &DataPayload<ExportMarker>,
        metadata: FlushMetadata,
    ) -> Result<(), DataError> {
        self.0
            .iter()
            .try_for_each(|e| e.flush_singleton(marker, payload, metadata))
    }

    fn flush(&self, marker: DataMarkerInfo, metadata: FlushMetadata) -> Result<(), DataError> {
        self.0.iter().try_for_each(|e| e.flush(marker, metadata))
    }

    fn close(&mut self) -> Result<ExporterCloseMetadata, DataError> {
        Ok(ExporterCloseMetadata(Some(Box::new(
            self.0.iter_mut().try_fold(vec![], |mut m, e| {
                m.push(e.close()?.0);
                Ok::<_, DataError>(m)
            })?,
        ))))
    }
}
