import { DarwinBashVariant, LinuxBashVariant } from "./types.js";

export const LINUX_BASH_VARIANTS: LinuxBashVariant[] = [
  { name: "ubuntu-24.04", ids: ["ubuntu"], versions: ["24.04"] },
  { name: "ubuntu-22.04", ids: ["ubuntu"], versions: ["22.04"] },
  { name: "ubuntu-20.04", ids: ["ubuntu"], versions: ["20.04"] },
  { name: "debian-12", ids: ["debian"], versions: ["12"] },
  { name: "debian-11", ids: ["debian"], versions: ["11"] },
  { name: "centos-9", ids: ["centos", "rhel", "rocky", "almalinux"], versions: ["9"] },
];

export const DARWIN_BASH_VARIANTS: DarwinBashVariant[] = [
  { name: "macos-15", minDarwin: 24 },
  { name: "macos-14", minDarwin: 23 },
  { name: "macos-13", minDarwin: 22 },
];
