export type LinuxBashVariant = {
  name: string;
  ids: string[];
  versions: string[];
};

export type DarwinBashVariant = {
  name: string;
  minDarwin: number;
};

export type OsReleaseInfo = {
  id: string;
  idLike: string[];
  versionId: string;
};

export type BashSelection = {
  path: string;
  variant: string;
};
