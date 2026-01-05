#!/bin/bash
# Build script for .rpm package

set -e

VERSION=${1:-0.1.0}
ARCH=$(rpm --eval '%{_arch}')

# Create RPM build directories
mkdir -p ~/rpmbuild/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}

# Build binary
cargo build --release --package zrc-agent

# Create spec file
cat > ~/rpmbuild/SPECS/zrc-agent.spec <<EOF
Name:           zrc-agent
Version:        ${VERSION}
Release:        1%{?dist}
Summary:        Zippy Remote Control Agent
License:        Apache-2.0 OR MIT
Source0:        %{name}-%{version}.tar.gz

%description
ZRC Agent provides remote desktop control capabilities.

%prep
%setup -q

%build
cargo build --release

%install
mkdir -p %{buildroot}/usr/bin
mkdir -p %{buildroot}/usr/lib/systemd/system
mkdir -p %{buildroot}/var/lib/zrc-agent

cp target/release/zrc-agent %{buildroot}/usr/bin/
cp deploy/zrc-agent.service %{buildroot}/usr/lib/systemd/system/

%post
systemctl daemon-reload
systemctl enable zrc-agent.service || true

%preun
systemctl stop zrc-agent.service || true
systemctl disable zrc-agent.service || true

%files
/usr/bin/zrc-agent
/usr/lib/systemd/system/zrc-agent.service
/var/lib/zrc-agent

%changelog
* $(date +"%a %b %d %Y") ZRC Team <zrc@example.com> - ${VERSION}-1
- Initial package
EOF

# Build RPM
rpmbuild -ba ~/rpmbuild/SPECS/zrc-agent.spec

echo "Package built in ~/rpmbuild/RPMS/${ARCH}/"
