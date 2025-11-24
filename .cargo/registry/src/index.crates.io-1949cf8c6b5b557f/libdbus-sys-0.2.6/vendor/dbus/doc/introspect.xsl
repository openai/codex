<?xml version="1.0" encoding="ISO-8859-15"?>
<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform" xmlns="http://www.w3.org/1999/xhtml">

<!-- 
 Copyright (C) 2005 Lennart Poettering.

 Licensed under the Academic Free License version 2.1

 This program is free software; you can redistribute it and/or modify
 it under the terms of the GNU General Public License as published by
 the Free Software Foundation; either version 2 of the License, or
 (at your option) any later version.

 This program is distributed in the hope that it will be useful,
 but WITHOUT ANY WARRANTY; without even the implied warranty of
 MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 GNU General Public License for more details.

 You should have received a copy of the GNU General Public License
 along with this program; if not, write to the Free Software
 Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301  USA
-->

<!-- $Id$ -->

<xsl:output method="xml" version="1.0" encoding="iso-8859-15" doctype-public="-//W3C//DTD XHTML 1.0 Strict//EN" doctype-system="http://www.w3.org/TR/xhtml1/DTD/xhtml1-strict.dtd" indent="yes"/>

<xsl:template match="/">
  <html>
    <head>
      <title>DBUS Introspection data</title>
      <style type="text/css">
        body { color: black; background-color: white } 
        h1 { font-family: sans-serif }
        ul { list-style-type: none; margin-bottom: 10px }
        li { font-family: sans-serif }
        .keyword { font-style: italic }
        .type { font-weight: bold }
        .symbol { font-family: monospace }
        .interface { padding: 10px; margin: 10px }
      </style>
    </head>
    <body>
      <xsl:for-each select="node/interface">
        <div class="interface">
          <h1>
            <span class="keyword">interface</span><xsl:text> </xsl:text>
            <span class="symbol"><xsl:value-of select="@name"/></span>
          </h1>   
          
          <ul>

            <xsl:apply-templates select="annotation"/> 

            <xsl:for-each select="method|signal|property">
              <li>
                <span class="keyword"><xsl:value-of select="name()"/></span>
                <xsl:text> </xsl:text>
                <span class="symbol"><xsl:value-of select="@name"/></span>
                
                <ul>
                  <xsl:apply-templates select="annotation"/> 
                  <xsl:for-each select="arg">
                    <li>
                      <span class="keyword">
                        <xsl:choose>
                          <xsl:when test="@direction != &quot;&quot;">
                            <xsl:value-of select="@direction"/> 
                          </xsl:when>
                          <xsl:when test="name(..) = &quot;signal&quot;">
                            out
                          </xsl:when>
                          <xsl:otherwise>
                            in
                          </xsl:otherwise>
                        </xsl:choose>
                      </span>

                      <xsl:text> </xsl:text>
                      
                      <span class="type"><xsl:value-of select="@type"/></span><xsl:text> </xsl:text>
                      <span class="symbol"><xsl:value-of select="@name"/></span><xsl:text> </xsl:text>

                      <xsl:if test="annotation">
                        <ul>
                          <xsl:apply-templates select="annotation"/>
                        </ul>
                      </xsl:if>
                    </li>
                  </xsl:for-each>
                </ul>

              </li>
            </xsl:for-each>

          </ul>
        </div>
      </xsl:for-each>
    </body>
  </html>
</xsl:template>


<xsl:template match="annotation"> 
  <li>
    <span class="keyword">annotation</span><xsl:text> </xsl:text>
    <code><xsl:value-of select="@name"/></code><xsl:text> = </xsl:text>
    <code><xsl:value-of select="@value"/></code>
  </li>
</xsl:template>

</xsl:stylesheet>
