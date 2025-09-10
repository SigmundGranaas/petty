<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <!--
      Defines the page layout using standard XSL-FO vocabulary.
      This block is pre-parsed by the pipeline builder to configure the document.
      The runtime parser will then ignore this block.
    -->
    <fo:layout-master-set>
        <fo:simple-page-master master-name="cv-page"
                               page-width="8.5in"
                               page-height="11in"
                               margin-top="0.75in"
                               margin-bottom="0.75in"
                               margin-left="0.75in"
                               margin-right="0.75in"/>
    </fo:layout-master-set>

    <!--
      Defines reusable style sets using standard XSLT.
      These are also pre-parsed by the pipeline builder and converted into
      internal style definitions.
    -->
    <xsl:attribute-set name="name">
        <xsl:attribute name="font-size">28pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="color">#2c3e50</xsl:attribute>
        <xsl:attribute name="margin-bottom">4pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="title">
        <xsl:attribute name="font-size">16pt</xsl:attribute>
        <xsl:attribute name="color">#34495e</xsl:attribute>
        <xsl:attribute name="margin-bottom">12pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="contact-info">
        <xsl:attribute name="font-size">9pt</xsl:attribute>
        <xsl:attribute name="color">#7f8c8d</xsl:attribute>
        <xsl:attribute name="line-height">12pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="section-heading">
        <xsl:attribute name="font-size">14pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="color">#2980b9</xsl:attribute>
        <xsl:attribute name="margin-top">18pt</xsl:attribute>
        <xsl:attribute name="margin-bottom">8pt</xsl:attribute>
        <xsl:attribute name="border">1pt solid #2980b9</xsl:attribute>
        <xsl:attribute name="padding-bottom">4pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="job-title">
        <xsl:attribute name="font-size">11pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="company-info">
        <xsl:attribute name="font-size">11pt</xsl:attribute>
        <xsl:attribute name="font-style">italic</xsl:attribute>
        <xsl:attribute name="color">#555</xsl:attribute>
        <xsl:attribute name="margin-bottom">6pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="responsibilities">
        <xsl:attribute name="font-size">10pt</xsl:attribute>
        <xsl:attribute name="margin-left">15pt</xsl:attribute>
        <xsl:attribute name="margin-bottom">4pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="degree">
        <xsl:attribute name="font-size">11pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="institution">
        <xsl:attribute name="font-size">11pt</xsl:attribute>
        <xsl:attribute name="color">#555</xsl:attribute>
        <xsl:attribute name="margin-bottom">12pt</xsl:attribute>
    </xsl:attribute-set>

    <!--
      The root template. This defines the overall structure of the document.
      The <page-sequence> tag creates a new logical page from the root of the data.
    -->
    <xsl:template match="/">
        <document>
            <page-sequence select="/">
                <!-- Header Section -->
                <container style="header">
                    <text style="name"><xsl:value-of select="name"/></text>
                    <text style="title"><xsl:value-of select="title"/></text>
                    <text style="contact-info">
                        <xsl:value-of select="contact/email"/> | <xsl:value-of select="contact/phone"/> | <xsl:value-of select="contact/linkedin"/>
                    </text>
                    <text style="contact-info">
                        <xsl:value-of select="contact/github"/> | <xsl:value-of select="contact/location"/>
                    </text>
                </container>

                <!-- Summary Section -->
                <text style="section-heading">Summary</text>
                <text><xsl:value-of select="summary"/></text>

                <!-- Experience Section -->
                <text style="section-heading">Experience</text>
                <xsl:for-each select="experience">
                    <container>
                        <text style="job-title"><xsl:value-of select="title"/></text>
                        <text style="company-info"><xsl:value-of select="company"/> | <xsl:value-of select="location"/> | <xsl:value-of select="dates"/></text>
                        <xsl:for-each select="responsibilities">
                            <text style="responsibilities">- <xsl:value-of select="."/></text>
                        </xsl:for-each>
                    </container>
                </xsl:for-each>

                <!-- Education Section -->
                <text style="section-heading">Education</text>
                <xsl:for-each select="education">
                    <container>
                        <text style="degree"><xsl:value-of select="degree"/></text>
                        <text style="institution"><xsl:value-of select="institution"/> | <xsl:value-of select="dates"/></text>
                    </container>
                </xsl:for-each>

                <!-- Skills Section -->
                <text style="section-heading">Skills</text>
                <xsl:for-each select="skills">
                    <text>
                        <strong><xsl:value-of select="category"/>:</strong> <xsl:value-of select="list"/>
                    </text>
                </xsl:for-each>

                <!-- Projects Section -->
                <text style="section-heading">Projects</text>
                <xsl:for-each select="projects">
                    <container>
                        <text style="job-title"><xsl:value-of select="name"/></text>
                        <text style="company-info"><xsl:value-of select="url"/></text>
                        <text><xsl:value-of select="description"/></text>
                        <text><strong>Technologies:</strong> <xsl:value-of select="technologies"/></text>
                    </container>
                </xsl:for-each>

            </page-sequence>
        </document>
    </xsl:template>
</xsl:stylesheet>