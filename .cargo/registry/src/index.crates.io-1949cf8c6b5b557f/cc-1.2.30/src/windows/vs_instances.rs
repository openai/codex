use std::borrow::Cow;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::io::BufRead;
use std::path::PathBuf;

use crate::windows::setup_config::{EnumSetupInstances, SetupInstance};

pub enum VsInstance {
    Com(SetupInstance),
    Vswhere(VswhereInstance),
}

impl VsInstance {
    pub fn installation_name(&self) -> Option<Cow<str>> {
        match self {
            VsInstance::Com(s) => s
                .installation_name()
                .ok()
                .and_then(|s| s.into_string().ok())
                .map(Cow::from),
            VsInstance::Vswhere(v) => v.map.get("installationName").map(Cow::from),
        }
    }

    pub fn installation_path(&self) -> Option<PathBuf> {
        match self {
            VsInstance::Com(s) => s.installation_path().ok().map(PathBuf::from),
            VsInstance::Vswhere(v) => v.map.get("installationPath").map(PathBuf::from),
        }
    }

    pub fn installation_version(&self) -> Option<Cow<str>> {
        match self {
            VsInstance::Com(s) => s
                .installation_version()
                .ok()
                .and_then(|s| s.into_string().ok())
                .map(Cow::from),
            VsInstance::Vswhere(v) => v.map.get("installationVersion").map(Cow::from),
        }
    }
}

pub enum VsInstances {
    ComBased(EnumSetupInstances),
    VswhereBased(VswhereInstance),
}

impl IntoIterator for VsInstances {
    type Item = VsInstance;
    #[allow(bare_trait_objects)]
    type IntoIter = Box<Iterator<Item = Self::Item>>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            VsInstances::ComBased(e) => {
                Box::new(e.into_iter().filter_map(Result::ok).map(VsInstance::Com))
            }
            VsInstances::VswhereBased(v) => Box::new(std::iter::once(VsInstance::Vswhere(v))),
        }
    }
}

#[derive(Debug)]
pub struct VswhereInstance {
    map: HashMap<String, String>,
}

impl TryFrom<&Vec<u8>> for VswhereInstance {
    type Error = &'static str;

    fn try_from(output: &Vec<u8>) -> Result<Self, Self::Error> {
        let map: HashMap<_, _> = output
            .lines()
            .map_while(Result::ok)
            .filter_map(|s| {
                let mut splitn = s.splitn(2, ": ");
                Some((splitn.next()?.to_owned(), splitn.next()?.to_owned()))
            })
            .collect();

        if !map.contains_key("installationName")
            || !map.contains_key("installationPath")
            || !map.contains_key("installationVersion")
        {
            return Err("required properties not found");
        }

        Ok(Self { map })
    }
}

#[cfg(test)]
mod tests_ {
    use std::borrow::Cow;
    use std::convert::TryFrom;
    use std::path::PathBuf;

    #[test]
    fn it_parses_vswhere_output_correctly() {
        let output = br"instanceId: 58104422
installDate: 21/02/2021 21:50:33
installationName: VisualStudio/16.9.2+31112.23
installationPath: C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools
installationVersion: 16.9.31112.23
productId: Microsoft.VisualStudio.Product.BuildTools
productPath: C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools\Common7\Tools\LaunchDevCmd.bat
state: 4294967295
isComplete: 1
isLaunchable: 1
isPrerelease: 0
isRebootRequired: 0
displayName: Visual Studio Build Tools 2019
description: The Visual Studio Build Tools allows you to build native and managed MSBuild-based applications without requiring the Visual Studio IDE. There are options to install the Visual C++ compilers and libraries, MFC, ATL, and C++/CLI support.
channelId: VisualStudio.16.Release
channelUri: https://aka.ms/vs/16/release/channel
enginePath: C:\Program Files (x86)\Microsoft Visual Studio\Installer\resources\app\ServiceHub\Services\Microsoft.VisualStudio.Setup.Service
releaseNotes: https://docs.microsoft.com/en-us/visualstudio/releases/2019/release-notes-v16.9#16.9.2
thirdPartyNotices: https://go.microsoft.com/fwlink/?LinkId=660909
updateDate: 2021-03-17T21:16:46.5963702Z
catalog_buildBranch: d16.9
catalog_buildVersion: 16.9.31112.23
catalog_id: VisualStudio/16.9.2+31112.23
catalog_localBuild: build-lab
catalog_manifestName: VisualStudio
catalog_manifestType: installer
catalog_productDisplayVersion: 16.9.2
catalog_productLine: Dev16
catalog_productLineVersion: 2019
catalog_productMilestone: RTW
catalog_productMilestoneIsPreRelease: False
catalog_productName: Visual Studio
catalog_productPatchVersion: 2
catalog_productPreReleaseMilestoneSuffix: 1.0
catalog_productSemanticVersion: 16.9.2+31112.23
catalog_requiredEngineVersion: 2.9.3365.38425
properties_campaignId: 156063665.1613940062
properties_channelManifestId: VisualStudio.16.Release/16.9.2+31112.23
properties_nickname: 
properties_setupEngineFilePath: C:\Program Files (x86)\Microsoft Visual Studio\Installer\vs_installershell.exe
"
        .to_vec();

        let vswhere_instance = super::VswhereInstance::try_from(&output);
        assert!(vswhere_instance.is_ok());

        let vs_instance = super::VsInstance::Vswhere(vswhere_instance.unwrap());
        assert_eq!(
            vs_instance.installation_name(),
            Some(Cow::from("VisualStudio/16.9.2+31112.23"))
        );
        assert_eq!(
            vs_instance.installation_path(),
            Some(PathBuf::from(
                r"C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools"
            ))
        );
        assert_eq!(
            vs_instance.installation_version(),
            Some(Cow::from("16.9.31112.23"))
        );
    }

    #[test]
    fn it_returns_an_error_for_empty_output() {
        let output = b"".to_vec();

        let vswhere_instance = super::VswhereInstance::try_from(&output);

        assert!(vswhere_instance.is_err());
    }

    #[test]
    fn it_returns_an_error_for_output_consisting_of_empty_lines() {
        let output = br"

"
        .to_vec();

        let vswhere_instance = super::VswhereInstance::try_from(&output);

        assert!(vswhere_instance.is_err());
    }

    #[test]
    fn it_returns_an_error_for_output_without_required_properties() {
        let output = br"instanceId: 58104422
installDate: 21/02/2021 21:50:33
productId: Microsoft.VisualStudio.Product.BuildTools
productPath: C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools\Common7\Tools\LaunchDevCmd.bat
"
        .to_vec();

        let vswhere_instance = super::VswhereInstance::try_from(&output);

        assert!(vswhere_instance.is_err());
    }
}
