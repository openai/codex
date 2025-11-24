<xsl:stylesheet
    xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
    xmlns:fo="http://www.w3.org/1999/XSL/Format"
    version="1.0">

<xsl:output method="xml" version="1.0" indent="yes"/>

<xsl:param name="prefix"></xsl:param>

<xsl:template match="/">
  <book title="D-Bus: A system for interprocess communication"
        name="dbus"
        link="{$prefix}/api/index.html"
        xmlns="http://www.devhelp.net/book"
        version="2"
        online="https://dbus.freedesktop.org/doc/"
        author="D-Bus contributors"
        language="c"
        >
  <chapters>
     <sub name="Tutorial" link="{$prefix}dbus-tutorial.html"/>
     <sub name="FAQ" link="{$prefix}dbus-faq.html"/>
     <sub name="Specification" link="{$prefix}dbus-specification.html"/>
     <sub name="API Reference" link="{$prefix}api/index.html"/>
  </chapters>

  <functions>
    <xsl:apply-templates select="doxygenindex/compound[@kind='group']/member[@kind='function']"/>
  </functions>
  </book>
</xsl:template>

<xsl:template match="member">
  <xsl:param name="name"><xsl:value-of select="name"/></xsl:param>
  <xsl:param name="refid"><xsl:value-of select="@refid"/></xsl:param>
  <xsl:param name="before"><xsl:value-of select="substring-before($refid,'_1')"/></xsl:param>
  <xsl:param name="after"><xsl:value-of select="substring-after($refid,'_1')"/></xsl:param>
  <xsl:param name="link"><xsl:value-of select="$before"/>.html#<xsl:value-of select="$after"/></xsl:param>
  <xsl:if test="starts-with($name,'dbus') or starts-with($name, 'DBus')">
    <xsl:if test="starts-with($refid,'group__') and contains($refid, '_1')">
       <keyword xmlns="http://www.devhelp.net/book" type="function" name="{$name}" link="{$prefix}api/{$link}"/>
    </xsl:if>
  </xsl:if>
</xsl:template>

</xsl:stylesheet>
