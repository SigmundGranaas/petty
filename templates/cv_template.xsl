<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <!-- =============================== -->
    <!-- Document Layout & Page Master   -->
    <!-- =============================== -->
    <fo:layout-master-set>
        <fo:simple-page-master master-name="A4-portrait"
                               page-height="842pt" page-width="595pt"
                               margin-top="40pt" margin-bottom="40pt"
                               margin-left="40pt" margin-right="40pt">
        </fo:simple-page-master>
    </fo:layout-master-set>

    <!-- =============================== -->
    <!--          Style Definitions      -->
    <!-- =============================== -->

    <!-- Header & Base Styles -->
    <xsl:attribute-set name="name">
        <xsl:attribute name="font-size">28pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="color">#142850</xsl:attribute>
        <xsl:attribute name="margin-bottom">4pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="title">
        <xsl:attribute name="font-size">16pt</xsl:attribute>
        <xsl:attribute name="color">#27496d</xsl:attribute>
        <xsl:attribute name="margin-bottom">12pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="contact-info">
        <xsl:attribute name="font-size">10pt</xsl:attribute>
        <xsl:attribute name="text-align">right</xsl:attribute>
        <xsl:attribute name="line-height">14pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="link">
        <xsl:attribute name="color">#00909e</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="bold">
        <xsl:attribute name="font-weight">bold</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="italic">
        <xsl:attribute name="font-style">italic</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="hr">
        <xsl:attribute name="height">1pt</xsl:attribute>
        <xsl:attribute name="background-color">#dae1e7</xsl:attribute>
        <xsl:attribute name="margin-top">15pt</xsl:attribute>
        <xsl:attribute name="margin-bottom">15pt</xsl:attribute>
    </xsl:attribute-set>

    <!-- Section Styles -->
    <xsl:attribute-set name="section-title">
        <xsl:attribute name="font-size">14pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="color">#142850</xsl:attribute>
        <xsl:attribute name="margin-bottom">10pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="body-text">
        <xsl:attribute name="font-size">10pt</xsl:attribute>
        <xsl:attribute name="line-height">14pt</xsl:attribute>
        <xsl:attribute name="margin-bottom">20pt</xsl:attribute>
    </xsl:attribute-set>

    <!-- Experience & Education Styles -->
    <xsl:attribute-set name="item-header">
        <xsl:attribute name="font-size">11pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="item-subheader">
        <xsl:attribute name="font-size">10pt</xsl:attribute>
        <xsl:attribute name="font-style">italic</xsl:attribute>
        <xsl:attribute name="color">#555555</xsl:attribute>
        <xsl:attribute name="margin-bottom">8pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="item-dates">
        <xsl:attribute name="font-size">10pt</xsl:attribute>
        <xsl:attribute name="font-style">italic</xsl:attribute>
        <xsl:attribute name="text-align">right</xsl:attribute>
        <xsl:attribute name="color">#555555</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="list">
        <xsl:attribute name="margin-left">15pt</xsl:attribute>
        <xsl:attribute name="margin-bottom">20pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="list-item">
        <xsl:attribute name="font-size">10pt</xsl:attribute>
        <xsl:attribute name="line-height">14pt</xsl:attribute>
        <xsl:attribute name="margin-bottom">4pt</xsl:attribute>
    </xsl:attribute-set>

    <!-- Internal styles for list item layout -->
    <xsl:attribute-set name="list-item-bullet">
        <xsl:attribute name="width">15pt</xsl:attribute>
        <xsl:attribute name="margin-bottom">0pt</xsl:attribute>
    </xsl:attribute-set>
    <xsl:attribute-set name="list-item-body">
        <xsl:attribute name="margin-bottom">0pt</xsl:attribute>
    </xsl:attribute-set>

    <!-- Skills Styles -->
    <xsl:attribute-set name="skill-category">
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="font-size">10pt</xsl:attribute>
        <xsl:attribute name="width">100pt</xsl:attribute>
        <xsl:attribute name="margin-right">10pt</xsl:attribute>
        <xsl:attribute name="margin-bottom">5pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="skill-list">
        <xsl:attribute name="font-size">10pt</xsl:attribute>
        <xsl:attribute name="margin-bottom">5pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="job-dates">
        <xsl:attribute name="text-align">right</xsl:attribute>
        <xsl:attribute name="font-style">italic</xsl:attribute>
        <xsl:attribute name="color">#555555</xsl:attribute>
    </xsl:attribute-set>

    <!-- =============================== -->
    <!--           Root Template         -->
    <!-- =============================== -->
    <xsl:template match="/">
        <document>
            <page-sequence select=".">

                <!-- HEADER -->
                <flex-container direction="row">
                    <container style="width: 70%;">
                        <text style="name"><xsl:value-of select="name"/></text>
                        <text style="title"><xsl:value-of select="title"/></text>
                    </container>
                    <container style="width: 30%;">
                        <text style="contact-info">
                            <link href="mailto:{email}" style="link"><xsl:value-of select="email"/></link><br/>
                            <xsl:value-of select="phone"/><br/>
                            <xsl:value-of select="location"/><br/>
                            <link href="https://{linkedin}" style="link"><xsl:value-of select="linkedin"/></link><br/>
                            <link href="https://{github}" style="link"><xsl:value-of select="github"/></link>
                        </text>
                    </container>
                </flex-container>

                <!-- SUMMARY -->
                <container>
                    <text style="section-title">Summary</text>
                    <rectangle style="hr"/>
                    <text style="body-text"><xsl:value-of select="summary"/></text>
                </container>

                <!-- EXPERIENCE -->
                <container>
                    <text style="section-title">Experience</text>
                    <rectangle style="hr"/>
                    <xsl:for-each select="experience">
                        <container>
                            <flex-container direction="row">
                                <text style="job-title"><xsl:value-of select="title"/></text>
                                <text style="job-dates"><xsl:value-of select="dates"/></text>
                            </flex-container>
                            <text style="item-subheader">
                                <b><xsl:value-of select="company"/></b> - <i><xsl:value-of select="location"/></i>
                            </text>
                            <list style="list">
                                <xsl:for-each select="responsibilities">
                                    <list-item style="list-item">
                                        <text><xsl:value-of select="."/></text>
                                    </list-item>
                                </xsl:for-each>
                            </list>
                        </container>
                    </xsl:for-each>
                </container>

                <!-- SKILLS -->
                <container>
                    <text style="section-title">Skills</text>
                    <rectangle style="hr"/>
                    <xsl:for-each select="skills">
                        <flex-container direction="row">
                            <text style="skill-category"><xsl:value-of select="category"/>:</text>
                            <text style="skill-list"><xsl:value-of select="list"/></text>
                        </flex-container>
                    </xsl:for-each>
                </container>

                <page-break/>

                <!-- EDUCATION -->
                <container>
                    <text style="section-title">Education</text>
                    <rectangle style="hr"/>
                    <xsl:for-each select="education">
                        <container style="margin-bottom: 15pt;">
                            <flex-container direction="row">
                                <container style="width: 75%;">
                                    <text style="item-header"><xsl:value-of select="degree"/></text>
                                </container>
                                <container style="width: 25%;">
                                    <text style="item-dates"><xsl:value-of select="dates"/></text>
                                </container>
                            </flex-container>
                            <text style="item-subheader"><xsl:value-of select="institution"/></text>
                        </container>
                    </xsl:for-each>
                </container>

                <!-- PROJECTS -->
                <container>
                    <text style="section-title">Projects</text>
                    <rectangle style="hr"/>
                    <xsl:for-each select="projects">
                        <container>
                            <flex-container direction="row">
                                <container style="width: 75%;">
                                    <text style="item-header"><xsl:value-of select="name"/></text>
                                </container>
                                <container style="width: 25%;">
                                    <text style="item-dates">
                                        <link href="https://{url}" style="link"><xsl:value-of select="url"/></link>
                                    </text>
                                </container>
                            </flex-container>
                            <text style="body-text"><xsl:value-of select="description"/></text>
                        </container>
                    </xsl:for-each>
                </container>

            </page-sequence>
        </document>
    </xsl:template>
</xsl:stylesheet>