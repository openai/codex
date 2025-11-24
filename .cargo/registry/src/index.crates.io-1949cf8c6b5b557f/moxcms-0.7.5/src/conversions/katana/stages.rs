/*
 * // Copyright (c) Radzivon Bartoshyk 6/2025. All rights reserved.
 * //
 * // Redistribution and use in source and binary forms, with or without modification,
 * // are permitted provided that the following conditions are met:
 * //
 * // 1.  Redistributions of source code must retain the above copyright notice, this
 * // list of conditions and the following disclaimer.
 * //
 * // 2.  Redistributions in binary form must reproduce the above copyright notice,
 * // this list of conditions and the following disclaimer in the documentation
 * // and/or other materials provided with the distribution.
 * //
 * // 3.  Neither the name of the copyright holder nor the names of its
 * // contributors may be used to endorse or promote products derived from
 * // this software without specific prior written permission.
 * //
 * // THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
 * // AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * // IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 * // DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
 * // FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 * // DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 * // SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
 * // CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
 * // OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
 * // OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */
use crate::{CmsError, TransformExecutor};
use std::marker::PhantomData;

/// W storage working data type
/// I input/output data type
pub(crate) trait KatanaInitialStage<W, I> {
    fn to_pcs(&self, input: &[I]) -> Result<Vec<W>, CmsError>;
}

/// W storage working data type
/// I input/output data type
pub(crate) trait KatanaFinalStage<W, I> {
    fn to_output(&self, src: &mut [W], dst: &mut [I]) -> Result<(), CmsError>;
}

/// W storage working data type
pub(crate) trait KatanaIntermediateStage<W> {
    fn stage(&self, input: &mut Vec<W>) -> Result<Vec<W>, CmsError>;
}

pub(crate) struct BlackholeIntermediateStage<W> {
    pub(crate) _phantom: PhantomData<W>,
}

impl<W> KatanaIntermediateStage<W> for BlackholeIntermediateStage<W> {
    fn stage(&self, input: &mut Vec<W>) -> Result<Vec<W>, CmsError> {
        Ok(std::mem::take(input))
    }
}

/// I input/output data type
pub(crate) trait KatanaPostFinalizationStage<I> {
    fn finalize(&self, src: &[I], dst: &mut [I]) -> Result<(), CmsError>;
}

/// W storage working data type
/// I input/output data type
pub(crate) struct Katana<W, I> {
    pub(crate) initial_stage: Box<dyn KatanaInitialStage<W, I> + Send + Sync>,
    pub(crate) final_stage: Box<dyn KatanaFinalStage<W, I> + Sync + Send>,
    pub(crate) stages: Vec<Box<dyn KatanaIntermediateStage<W> + Send + Sync>>,
    pub(crate) post_finalization: Vec<Box<dyn KatanaPostFinalizationStage<I> + Send + Sync>>,
}

impl<W, I: Copy + Default> TransformExecutor<I> for Katana<W, I> {
    fn transform(&self, src: &[I], dst: &mut [I]) -> Result<(), CmsError> {
        let mut working_vec = self.initial_stage.to_pcs(src)?;
        for stage in self.stages.iter() {
            working_vec = stage.stage(&mut working_vec)?;
        }
        self.final_stage.to_output(&mut working_vec, dst)?;
        for finalization in self.post_finalization.iter() {
            finalization.finalize(src, dst)?;
        }
        Ok(())
    }
}
