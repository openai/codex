dbusdatadir=$(datadir)/dbus-1
legacydbusdatadir=$(sysconfdir)/dbus-1
dbus_daemon_execdir = $(DBUS_DAEMONDIR)
# Always lib, even if ${libdir} is lib64 or lib/x86_64-linux-gnu
systemdtmpfilesdir = $(prefix)/lib/tmpfiles.d
systemdsysusersdir = $(prefix)/lib/sysusers.d

DBUS_BUS_LIBS = \
	$(CODE_COVERAGE_LIBS) \
	$(EXPAT_LIBS) \
	$(SELINUX_LIBS) \
	$(APPARMOR_LIBS) \
	$(THREAD_LIBS) \
	$(ADT_LIBS) \
	$(NETWORK_libs) \
	$(NULL)

DBUS_LAUNCHER_LIBS = \
	$(CODE_COVERAGE_LIBS) \
	$(EXPAT_LIBS) \
	$(THREAD_LIBS) \
	$(NETWORK_libs) \
	$(NULL)

AM_CPPFLAGS = \
	$(CODE_COVERAGE_CPPFLAGS) \
	-I$(top_srcdir) \
	$(DBUS_STATIC_BUILD_CPPFLAGS) \
	$(EXPAT_CFLAGS) \
	$(APPARMOR_CFLAGS) \
	-DDBUS_SYSTEM_CONFIG_FILE=\""$(dbusdatadir)/system.conf"\" \
	-DDBUS_RUNSTATEDIR=\""$(runstatedir)"\" \
	-DDBUS_COMPILATION \
	$(NULL)

# if assertions are enabled, improve backtraces
AM_LDFLAGS = @R_DYNAMIC_LDFLAG@

AM_CFLAGS = \
	$(CODE_COVERAGE_CFLAGS) \
	$(SANITIZE_CFLAGS) \
	$(NULL)

EFENCE=

EXTRA_DIST =					\
	session.conf.in				\
	system.conf.in				\
	legacy-config/session.conf.in		\
	legacy-config/system.conf.in		\
	org.freedesktop.dbus-session.plist.in	\
	example-system-enable-stats.conf.in	\
	example-session-disable-stats.conf.in	\
	$(NULL)

dbusdata_DATA = session.conf
legacydbusdata_DATA = legacy-config/session.conf

if DBUS_UNIX
dbusdata_DATA += system.conf
legacydbusdata_DATA += legacy-config/system.conf
endif

examplesdir = ${docdir}/examples
examples_DATA = \
	example-system-enable-stats.conf	\
	example-session-disable-stats.conf	\
	$(NULL)

dist_examples_DATA = \
	example-system-hardening-without-traditional-activation.conf	\
	$(NULL)

if DBUS_ENABLE_LAUNCHD
agentdir=$(LAUNCHD_AGENT_DIR)
agent_DATA=org.freedesktop.dbus-session.plist
endif

if DBUS_BUS_ENABLE_KQUEUE
DIR_WATCH_SOURCE=dir-watch-kqueue.c
else
if DBUS_BUS_ENABLE_INOTIFY
DIR_WATCH_SOURCE=dir-watch-inotify.c
else
DIR_WATCH_SOURCE=dir-watch-default.c
endif
endif

noinst_LTLIBRARIES = libdbus-daemon-internal.la

libdbus_daemon_internal_la_SOURCES =		\
	activation.c				\
	activation.h				\
	activation-exit-codes.h			\
	apparmor.c				\
	apparmor.h				\
	audit.c					\
	audit.h					\
	bus.c					\
	bus.h					\
	config-loader-expat.c			\
	config-parser.c				\
	config-parser.h				\
	config-parser-common.c			\
	config-parser-common.h			\
	connection.c				\
	connection.h				\
	containers.c				\
	containers.h				\
	desktop-file.c				\
	desktop-file.h				\
	$(DIR_WATCH_SOURCE)			\
	dir-watch.h				\
	dispatch.c				\
	dispatch.h				\
	driver.c				\
	driver.h				\
	expirelist.c				\
	expirelist.h				\
	policy.c				\
	policy.h				\
	selinux.h				\
	selinux.c				\
	services.c				\
	services.h				\
	signals.c				\
	signals.h				\
	stats.c					\
	stats.h					\
	test.c					\
	test.h					\
	utils.c					\
	utils.h					\
	$(NULL)

libdbus_daemon_internal_la_LIBADD = \
	$(top_builddir)/dbus/libdbus-1.la \
	$(top_builddir)/dbus/libdbus-internal.la \
	$(EFENCE) \
	$(DBUS_BUS_LIBS) \
	$(NULL)

dbus_daemon_SOURCES = main.c
dbus_daemon_LDADD = libdbus-daemon-internal.la

liblaunch_helper_internal_la_SOURCES =		\
	config-loader-expat.c			\
	config-parser-common.c			\
	config-parser-common.h			\
	config-parser-trivial.c			\
	config-parser-trivial.h			\
	desktop-file.c				\
	desktop-file.h				\
	utils.c					\
	utils.h					\
	activation-exit-codes.h			\
	activation-helper.h	  		\
	$(NULL)

## This is the installed launch helper with the setuid checks
## All files that have special cases #ifdef ACTIVATION_LAUNCHER_TEST must
## be listed here and included in test/bus/launch-helper-for-tests.c,
## not in liblaunch-helper-internal.la.
dbus_daemon_launch_helper_SOURCES = \
	activation-helper.c \
	activation-helper-bin.c \
	$(NULL)
dbus_daemon_launch_helper_LDADD = liblaunch-helper-internal.la

liblaunch_helper_internal_la_LIBADD = \
	$(top_builddir)/dbus/libdbus-1.la \
	$(top_builddir)/dbus/libdbus-internal.la \
	$(DBUS_LAUNCHER_LIBS)

noinst_PROGRAMS =
dbus_daemon_exec_PROGRAMS = dbus-daemon
if DBUS_UNIX
if ENABLE_TRADITIONAL_ACTIVATION
noinst_LTLIBRARIES += liblaunch-helper-internal.la
libexec_PROGRAMS = dbus-daemon-launch-helper
endif ENABLE_TRADITIONAL_ACTIVATION
endif DBUS_UNIX

install-data-hook:
	$(mkinstalldirs) $(DESTDIR)$(dbusdatadir)/session.d
	$(mkinstalldirs) $(DESTDIR)$(dbusdatadir)/services
if DBUS_UNIX
	$(mkinstalldirs) $(DESTDIR)$(localstatedir)/run/dbus
	$(mkinstalldirs) $(DESTDIR)$(dbusdatadir)/system.d
	$(mkinstalldirs) $(DESTDIR)$(dbusdatadir)/system-services
endif
if HAVE_SYSTEMD
# Install dbus.socket as default implementation of a D-Bus stack.
# Deliberately not using $(LN_S) here: ln -fs is not universally portable,
# but neither is systemd, so it's OK to assume here that ln complies with SUS.
# Unconditionally enable D-Bus on systemd installations
	$(mkinstalldirs) $(DESTDIR)$(systemdsystemunitdir)/sockets.target.wants
	ln -fs ../dbus.socket $(DESTDIR)$(systemdsystemunitdir)/sockets.target.wants/dbus.socket
	$(mkinstalldirs) $(DESTDIR)$(systemdsystemunitdir)/multi-user.target.wants
	ln -fs ../dbus.service $(DESTDIR)$(systemdsystemunitdir)/multi-user.target.wants/dbus.service
endif
if DBUS_ENABLE_USER_SESSION
	$(mkinstalldirs) $(DESTDIR)$(systemduserunitdir)/sockets.target.wants
	ln -fs ../dbus.socket $(DESTDIR)$(systemduserunitdir)/sockets.target.wants/dbus.socket
endif

if DBUS_UNIX
if ENABLE_TRADITIONAL_ACTIVATION
install-exec-hook:
	if test `id -u` -eq 0; then \
		chown root:$(DBUS_USER) $(DESTDIR)$(libexecdir)/dbus-daemon-launch-helper$(EXEEXT); \
		chmod 4750 $(DESTDIR)$(libexecdir)/dbus-daemon-launch-helper$(EXEEXT); \
	else \
		echo "Not installing $(DESTDIR)$(libexecdir)/dbus-daemon-launch-helper binary setuid!"; \
		echo "You'll need to manually set permissions to root:$(DBUS_USER) and permissions 4750"; \
	fi
endif ENABLE_TRADITIONAL_ACTIVATION
endif DBUS_UNIX

EXTRA_DIST += \
	dbus.service.in \
	dbus.socket.in \
	systemd-user/dbus.service.in \
	systemd-user/dbus.socket.in \
	sysusers.d/dbus.conf.in \
	tmpfiles.d/dbus.conf.in \
	$(NULL)

if HAVE_SYSTEMD
systemdsystemunit_DATA = \
	dbus.service \
	dbus.socket

nodist_systemdsysusers_DATA = \
	sysusers.d/dbus.conf

nodist_systemdtmpfiles_DATA = \
	tmpfiles.d/dbus.conf \
	$(NULL)
endif

if DBUS_ENABLE_USER_SESSION
systemduserunit_DATA = \
	systemd-user/dbus.service \
	systemd-user/dbus.socket \
	$(NULL)
endif

# Add rules for code-coverage testing, as defined by AX_CODE_COVERAGE
include $(top_srcdir)/aminclude_static.am
