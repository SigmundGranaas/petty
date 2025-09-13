<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform" xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <!-- Page Master: Defines page size and margins. -->
    <fo:layout-master-set>
        <fo:simple-page-master master-name="A4-CV"
                               page-width="8.5in"
                               page-height="11in"
                               margin-top="0.75in"
                               margin-bottom="0.75in"
                               margin-left="0.75in"
                               margin-right="0.75in">
        </fo:simple-page-master>
    </fo:layout-master-set>

    <!-- Style Definitions: These are pre-parsed by the engine. -->
    <xsl:attribute-set name="h1">
        <xsl:attribute name="font-size">36pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="color">#003366</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="subtitle">
        <xsl:attribute name="font-size">14pt</xsl:attribute>
        <xsl:attribute name="color">#333333</xsl:attribute>
        <xsl:attribute name="margin-bottom">20pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="h2">
        <xsl:attribute name="font-size">18pt</xsl:attribute>
        <xsl:attribute name="color">#003366</xsl:attribute>
        <xsl:attribute name="margin-top">20pt</xsl:attribute>
        <xsl:attribute name="margin-bottom">10pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="contact-info">
        <xsl:attribute name="text-align">right</xsl:attribute>
        <xsl:attribute name="font-size">10pt</xsl:attribute>
        <xsl:attribute name="line-height">14pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="item-title">
        <xsl:attribute name="font-size">12pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="item-subtitle">
        <xsl:attribute name="font-size">11pt</xsl:attribute>
        <xsl:attribute name="font-style">italic</xsl:attribute>
        <xsl:attribute name="color">#555555</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="item-dates">
        <xsl:attribute name="font-size">11pt</xsl:attribute>
        <xsl:attribute name="color">#555555</xsl:attribute>
        <xsl:attribute name="text-align">right</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="list-container">
        <xsl:attribute name="margin-left">20pt</xsl:attribute>
        <xsl:attribute name="margin-top">5pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="section-item">
        <xsl:attribute name="margin-bottom">15pt</xsl:attribute>
    </xsl:attribute-set>

    <!-- Main Template -->
    <xsl:template match="/">
        <page-sequence select=".">
            <!-- Header -->
            <flex-container>
                <container width="65%">
                    <text style="h1"><xsl:value-of select="name"/></text>
                    <text style="subtitle"><xsl:value-of select="title"/></text>
                </container>
                <container width="35%">
                    <text style="contact-info">
                        <xsl:value-of select="phone"/><br/>
                        <xsl:value-of select="location"/><br/>
                        <link href="mailto:{email}"><xsl:value-of select="email"/></link><br/>
                        <link href="https://{linkedin}"><xsl:value-of select="linkedin"/></link><br/>
                        <link href="https://{github}"><xsl:value-of select="github"/></link>
                    </text>
                </container>
            </flex-container>

            <!-- Summary -->
            <container>
                <text style="h2">Summary</text>
                <text><xsl:value-of select="summary"/></text>
            </container>

            <!-- Experience -->
            <container>
                <text style="h2">Experience</text>
                <xsl:for-each select="experience">
                    <container style="section-item">
                        <flex-container>
                            <container>
                                <text style="item-title"><xsl:value-of select="title"/></text>
                                <text style="item-subtitle"><xsl:value-of select="company"/> - <xsl:value-of select="location"/></text>
                            </container>
                            <container>
                                <text style="item-dates"><xsl:value-of select="dates"/></text>
                            </container>
                        </flex-container>
                        <list style="list-container">
                            <xsl:for-each select="responsibilities">
                                <list-item>
                                    <text><xsl:value-of select="."/></text>
                                </list-item>
                            </xsl:for-each>
                        </list>
                    </container>
                </xsl:for-each>
            </container>

            <!-- Skills -->
            <container>
                <text style="h2">Skills</text>
                <xsl:for-each select="skills">
                    <text>
                        <b><xsl:value-of select="category"/>:</b> <xsl:value-of select="list"/>
                    </text>
                </xsl:for-each>
            </container>

            <!-- Education -->
            <container>
                <text style="h2">Education</text>
                <xsl:for-each select="education">
                    <container style="section-item">
                        <flex-container>
                            <container>
                                <text style="item-title"><xsl:value-of select="institution"/></text>
                                <text style="item-subtitle"><xsl:value-of select="degree"/></text>
                            </container>
                            <container>
                                <text style="item-dates"><xsl:value-of select="dates"/></text>
                            </container>
                        </flex-container>
                    </container>
                </xsl:for-each>
            </container>

            <!-- Projects -->
            <container>
                <text style="h2">Projects</text>
                <xsl:for-each select="projects">
                    <container style="section-item">
                        <flex-container>
                            <container>
                                <text style="item-title"><xsl:value-of select="name"/></text>
                            </container>
                            <container>
                                <text style="item-dates">
                                    <link href="https://{url}"><xsl:value-of select="url"/></link>
                                </text>
                            </container>
                        </flex-container>
                        <text><xsl:value-of select="description"/></text>
                        <text>
                            <b>Technologies:</b> <xsl:value-of select="technologies"/>
                        </text>
                    </container>
                </xsl:for-each>
            </container>

        </page-sequence>
    </xsl:template>
</xsl:stylesheet>