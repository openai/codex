export type LinuxBashVariant = {
  name: string;
  ids: Array<string>;
  versions: Array<string>;
};

export type DarwinBashVariant = {
  name: string;
  minDarwin: number;
};

export type OsReleaseInfo = {
  id: string;
  idLike: Array<string>;
  versionId: string;
};

export type BashSelection = {
  path: string;
  variant: string;
};
