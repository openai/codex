# Try to find DocBook XSL stylesheet
# Once done, it will define:
#
#  DocBookXSL_FOUND - system has the required DocBook XML DTDs
#  DocBookXSL_DIR - the directory containing the stylesheets
#  used to process DocBook XML

# Copyright (c) 2010, Luigi Toscano, <luigi.toscano@tiscali.it>
#
# Redistribution and use in source and binary forms, with or without
# modification, are permitted provided that the following conditions
# are met:
# 1. Redistributions of source code must retain the above copyright
#    notice, this list of conditions and the following disclaimer.
# 2. Redistributions in binary form must reproduce the above copyright
#    notice, this list of conditions and the following disclaimer in the
#    documentation and/or other materials provided with the distribution.
# 3. Neither the name of the University nor the names of its contributors
#    may be used to endorse or promote products derived from this software
#    without specific prior written permission.
#
# THIS SOFTWARE IS PROVIDED BY THE REGENTS AND CONTRIBUTORS ``AS IS'' AND
# ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
# IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
# ARE DISCLAIMED.  IN NO EVENT SHALL THE REGENTS OR CONTRIBUTORS BE LIABLE
# FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
# DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS
# OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
# HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
# LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY
# OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF
# SUCH DAMAGE.

include(FeatureSummary)
set_package_properties(DocBookXSL PROPERTIES DESCRIPTION "DocBook XSL"
                       URL "http://docbook.sourceforge.net/release/xsl/current/"
                      )

set (STYLESHEET_PATH_LIST
    ${CMAKE_INSTALL_DATAROOTDIR}/xml/docbook/stylesheet/docbook-xsl
    ${CMAKE_INSTALL_DATAROOTDIR}/xml/docbook/xsl-stylesheets
    ${CMAKE_INSTALL_DATAROOTDIR}/sgml/docbook/xsl-stylesheets
    ${CMAKE_INSTALL_DATAROOTDIR}/xml/docbook/stylesheet/nwalsh/current
    ${CMAKE_INSTALL_DATAROOTDIR}/xml/docbook/stylesheet/nwalsh
    ${CMAKE_INSTALL_DATAROOTDIR}/xsl/docbook
    ${CMAKE_INSTALL_DATAROOTDIR}/xsl/docbook-xsl
    #for building on Mac with docbook-xsl installed by homebrew
    opt/docbook-xsl/docbook-xsl
)

find_path (DocBookXSL_DIR lib/lib.xsl
    PATHS ${CMAKE_SYSTEM_PREFIX_PATH}
    PATH_SUFFIXES ${STYLESHEET_PATH_LIST}
)

if (NOT DocBookXSL_DIR)
    # hacks for systems that put the version in the stylesheet dirs
    set (STYLESHEET_PATH_LIST)
    foreach (STYLESHEET_PREFIX_ITER ${CMAKE_SYSTEM_PREFIX_PATH})
        file(GLOB STYLESHEET_SUFFIX_ITER RELATIVE ${STYLESHEET_PREFIX_ITER}
            ${STYLESHEET_PREFIX_ITER}/share/xml/docbook/xsl-stylesheets-*
        )
        if (STYLESHEET_SUFFIX_ITER)
            list (APPEND STYLESHEET_PATH_LIST ${STYLESHEET_SUFFIX_ITER})
        endif ()
    endforeach ()

    find_path (DocBookXSL_DIR VERSION
        PATHS ${CMAKE_SYSTEM_PREFIX_PATH}
        PATH_SUFFIXES ${STYLESHEET_PATH_LIST}
    )
endif ()


include(FindPackageHandleStandardArgs)
find_package_handle_standard_args (DocBookXSL
    REQUIRED_VARS DocBookXSL_DIR
    FOUND_VAR DocBookXSL_FOUND)

#maintain backwards compatibility
set(DOCBOOKXSL_FOUND ${DocBookXSL_FOUND})
set(DOCBOOKXSL_DIR ${DocBookXSL_DIR})

mark_as_advanced (DocBookXSL_DIR)
