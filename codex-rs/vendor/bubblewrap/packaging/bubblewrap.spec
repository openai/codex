%global commit0 66d12bb23b04e201c5846e325f0b10930ed802f8
%global shortcommit0 %(c=%{commit0}; echo ${c:0:7})

Summary: Core execution tool for unprivileged containers
Name: bubblewrap
Version: 0
Release: 1%{?dist}
#VCS: git:https://github.com/projectatomic/bubblewrap
Source0: https://github.com/projectatomic/%{name}/archive/%{commit0}.tar.gz#/%{name}-%{shortcommit0}.tar.gz
License: LGPLv2+
URL: https://github.com/projectatomic/bubblewrap

BuildRequires: git
# We always run autogen.sh
BuildRequires: autoconf automake libtool
BuildRequires: libcap-devel
BuildRequires: pkgconfig(libselinux)
BuildRequires: libxslt
BuildRequires: docbook-style-xsl

%description
Bubblewrap (/usr/bin/bwrap) is a core execution engine for unprivileged
containers that works as a setuid binary on kernels without
user namespaces.

%prep
%autosetup -Sgit -n %{name}-%{version}

%build
env NOCONFIGURE=1 ./autogen.sh
%configure --disable-silent-rules --with-priv-mode=none

make %{?_smp_mflags}

%install
make install DESTDIR=$RPM_BUILD_ROOT INSTALL="install -p -c"
find $RPM_BUILD_ROOT -name '*.la' -delete

%files
%license COPYING
%doc README.md
%{_datadir}/bash-completion/completions/bwrap
%if (0%{?rhel} != 0 && 0%{?rhel} <= 7)
%attr(4755,root,root) %{_bindir}/bwrap
%else
%{_bindir}/bwrap
%endif
%{_mandir}/man1/*
