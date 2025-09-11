<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <!-- Page Layout and Margins -->
    <fo:layout-master-set>
        <fo:simple-page-master master-name="cv-page"
                               page-width="8.5in" page-height="11in"
                               margin-top="0.75in" margin-bottom="0.75in"
                               margin-left="1in" margin-right="1in"
                               footer-text="{{name}} | {{title}}"
                               footer-style="footer"/>
    </fo:layout-master-set>

    <!-- Style Definitions -->
    <xsl:attribute-set name="name-header">
        <xsl:attribute name="font-size">28pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
    </xsl:attribute-set>
    <xsl:attribute-set name="title-header">
        <xsl:attribute name="font-size">14pt</xsl:attribute>
        <xsl:attribute name="color">#555555</xsl:attribute>
    </xsl:attribute-set>
    <xsl:attribute-set name="contact-header">
        <xsl:attribute name="font-size">10pt</xsl:attribute>
        <xsl:attribute name="text-align">center</xsl:attribute>
        <xsl:attribute name="margin-bottom">20pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="h2">
        <xsl:attribute name="font-size">12pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="color">#2E3A87</xsl:attribute>
        <xsl:attribute name="margin-top">16pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="divider">
        <xsl:attribute name="height">1pt</xsl:attribute>
        <xsl:attribute name="background-color">#A9B3E0</xsl:attribute>
        <xsl:attribute name="margin-bottom">8pt</xsl:attribute>
        <xsl:attribute name="margin-top">2pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="p">
        <xsl:attribute name="font-size">10pt</xsl:attribute>
        <xsl:attribute name="line-height">14pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="job-title">
        <xsl:attribute name="font-size">11pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="margin-top">10pt</xsl:attribute>
        <xsl:attribute name="margin-bottom">0pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="company-info">
        <xsl:attribute name="font-size">10pt</xsl:attribute>
        <xsl:attribute name="font-style">italic</xsl:attribute>
        <xsl:attribute name="color">#555555</xsl:attribute>
        <xsl:attribute name="margin-bottom">8pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="bullet">
        <xsl:attribute name="font-size">10pt</xsl:attribute>
        <xsl:attribute name="line-height">14pt</xsl:attribute>
        <xsl:attribute name="margin-left">15pt</xsl:attribute>
        <xsl:attribute name="margin-bottom">4pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="bold">
        <xsl:attribute name="font-weight">bold</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="footer">
        <xsl:attribute name="font-size">9pt</xsl:attribute>
        <xsl:attribute name="color">#888888</xsl:attribute>
        <xsl:attribute name="text-align">center</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="link">
        <xsl:attribute name="color">#1a0dab</xsl:attribute>
    </xsl:attribute-set>

    <!-- Root Template -->
    <xsl:template match="/">
        <document>
            <page-sequence select=".">
                <!-- Header -->
                <container style="text-align: center; margin-bottom: 20pt;">
                    <text style="name-header"><xsl:value-of select="name"/></text>
                    <text style="title-header"><xsl:value-of select="title"/></text>
                    <text style="contact-header">
                        <xsl:value-of select="location"/> | <xsl:value-of select="email"/> | <xsl:value-of select="phone"/>
                    </text>
                </container>

                <!-- Summary -->
                <text style="h2">SUMMARY</text>
                <rectangle style="divider"/>
                <text style="p"><xsl:value-of select="summary"/></text>

                <!-- Experience -->
                <text style="h2">EXPERIENCE</text>
                <rectangle style="divider"/>
                <xsl:for-each select="experience">
                    <text style="job-title"><xsl:value-of select="title"/></text>
                    <text style="company-info"><xsl:value-of select="company"/> | <xsl:value-of select="location"/> | <xsl:value-of select="dates"/></text>
                    <xsl:for-each select="responsibilities">
                        <text style="bullet">• <xsl:value-of select="."/></text>
                    </xsl:for-each>
                </xsl:for-each>

                <!-- Education -->
                <text style="h2">EDUCATION</text>
                <rectangle style="divider"/>
                <xsl:for-each select="education">
                    <text style="job-title"><xsl:value-of select="degree"/></text>
                    <text style="company-info"><xsl:value-of select="institution"/> | <xsl:value-of select="dates"/></text>
                </xsl:for-each>

                <!-- Skills -->
                <text style="h2">TECHNICAL SKILLS</text>
                <rectangle style="divider"/>
                <table>
                    <columns>
                        <column width="25%"/>
                        <column width="75%"/>
                    </columns>
                    <tbody>
                        <xsl:for-each select="skills">
                            <row>
                                <cell><text style="bold"><xsl:value-of select="category"/></text></cell>
                                <cell><text><xsl:value-of select="list"/></text></cell>
                            </row>
                        </xsl:for-each>
                    </tbody>
                </table>

                <!-- Projects -->
                <text style="h2">PROJECTS</text>
                <rectangle style="divider"/>
                <xsl:for-each select="projects">
                    <container>
                        <link href="{url}" style="link"><text style="job-title"><xsl:value-of select="name"/></text></link>
                    </container>
                    <text style="p"><xsl:value-of select="description"/></text>
                    <text style="company-info">Technologies: <xsl:value-of select="technologies"/></text>
                </xsl:for-each>

            </page-sequence>
        </document>
    </xsl:template>
</xsl:stylesheet>