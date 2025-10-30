<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <!-- Page Layout -->
    <fo:simple-page-master page-width="8.5in" page-height="11in" margin="1in"/>

    <!-- Styles -->
    <xsl:attribute-set name="h1">
        <xsl:attribute name="font-size">24pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="margin-bottom">20pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="h2">
        <xsl:attribute name="font-size">18pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="margin-top">16pt</xsl:attribute>
        <xsl:attribute name="margin-bottom">8pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="h3">
        <xsl:attribute name="font-size">14pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="margin-top">12pt</xsl:attribute>
        <xsl:attribute name="margin-bottom">6pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="toc-style">
        <xsl:attribute name="border">1pt solid #ccc</xsl:attribute>
        <xsl:attribute name="padding">12pt</xsl:attribute>
        <xsl:attribute name="margin-bottom">24pt</xsl:attribute>
        <xsl:attribute name="background-color">#f9f9f9</xsl:attribute>
    </xsl:attribute-set>

    <!-- Main Template -->
    <xsl:template match="/*">
        <fo:block>
            <!-- Document Title -->
            <h1 use-attribute-sets="h1"><xsl:value-of select="documentTitle"/></h1>

            <!-- Table of Contents -->
            <h2 use-attribute-sets="h2">Table of Contents</h2>
            <toc use-attribute-sets="toc-style"/>

            <!-- Page Break after TOC -->
            <page-break/>

            <!-- Process Sections -->
            <xsl:apply-templates select="sections/item"/>
        </fo:block>
    </xsl:template>

    <!-- Section Template -->
    <xsl:template match="sections/item">
        <h2 use-attribute-sets="h2">
            <xsl:attribute name="id">
                <xsl:value-of select="generate-id(.)"/>
            </xsl:attribute>
            <xsl:value-of select="title"/>
        </h2>
        <p><xsl:value-of select="content"/></p>

        <!-- Process Subsections -->
        <xsl:apply-templates select="subsections/item"/>
    </xsl:template>

    <!-- Subsection Template -->
    <xsl:template match="subsections/item">
        <h3 use-attribute-sets="h3">
            <xsl:attribute name="id">
                <xsl:value-of select="generate-id(.)"/>
            </xsl:attribute>
            <xsl:value-of select="title"/>
        </h3>
        <p><xsl:value-of select="content"/></p>
    </xsl:template>

</xsl:stylesheet>