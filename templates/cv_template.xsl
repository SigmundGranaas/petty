<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform" xmlns:petty="https://docs.petty.rs/petty-ns">

    <!--
      =======================================================================
      1. PAGE LAYOUT AND STYLING DEFINITIONS
      These are pre-parsed by the engine to configure the document.
      =======================================================================
    -->

    <petty:page-layout size="Letter" margin="50pt" />

    <xsl:attribute-set name="name">
        <xsl:attribute name="font-size">28pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="color">#142850</xsl:attribute>
        <xsl:attribute name="margin-bottom">2pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="title">
        <xsl:attribute name="font-size">16pt</xsl:attribute>
        <xsl:attribute name="color">#27496d</xsl:attribute>
        <xsl:attribute name="margin-bottom">15pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="contact-info">
        <xsl:attribute name="font-size">9pt</xsl:attribute>
        <xsl:attribute name="color">#666666</xsl:attribute>
        <xsl:attribute name="line-height">12pt</xsl:attribute>
        <xsl:attribute name="margin-bottom">20pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="section-title">
        <xsl:attribute name="font-size">14pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="color">#27496d</xsl:attribute>
        <xsl:attribute name="margin-top">10pt</xsl:attribute>
        <xsl:attribute name="margin-bottom">8pt</xsl:attribute>
        <xsl:attribute name="border">1pt Solid #27496d</xsl:attribute>
        <xsl:attribute name="padding-bottom">4pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="job-title">
        <xsl:attribute name="font-size">11pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="margin-bottom">2pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="job-meta">
        <xsl:attribute name="font-size">10pt</xsl:attribute>
        <xsl:attribute name="font-style">Italic</xsl:attribute>
        <xsl:attribute name="color">#444444</xsl:attribute>
        <xsl:attribute name="margin-bottom">6pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="body-text">
        <xsl:attribute name="font-size">10pt</xsl:attribute>
        <xsl:attribute name="line-height">13pt</xsl:attribute>
        <xsl:attribute name="margin-bottom">10pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="list-item">
        <xsl:attribute name="font-size">10pt</xsl:attribute>
        <xsl:attribute name="line-height">14pt</xsl:attribute>
        <xsl:attribute name="margin-left">15pt</xsl:attribute>
        <xsl:attribute name="margin-bottom">4pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="skill-category">
        <xsl:attribute name="font-weight">bold</xsl:attribute>
    </xsl:attribute-set>

    <!--
      =======================================================================
      2. DOCUMENT CONTENT TEMPLATE
      This is the entry point for generating the PDF content from the data.
      =======================================================================
    -->

    <xsl:template match="/">
        <!--
          CORRECTED: All content-generating tags are now inside this
          <page-sequence>. It processes the root of the JSON data (select=".")
          and creates a single document from it.
        -->
        <page-sequence select=".">
            <!-- Header Section -->
            <container>
                <text style="name"><xsl:value-of select="/name"/></text>
                <text style="title"><xsl:value-of select="/title"/></text>
                <text style="contact-info">
                    <xsl:value-of select="concat(/contact/email, ' | ', /contact/phone, ' | ', /contact/location, '&#xA;')"/>
                    <xsl:value-of select="concat('LinkedIn: ', /contact/linkedin, ' | GitHub: ', /contact/github)"/>
                </text>
            </container>

            <!-- Summary Section -->
            <text style="section-title">Summary</text>
            <text style="body-text"><xsl:value-of select="/summary"/></text>

            <!-- Experience Section -->
            <text style="section-title">Professional Experience</text>
            <xsl:for-each select="/experience">
                <container style="body-text">
                    <text style="job-title"><xsl:value-of select="title"/></text>
                    <text style="job-meta">
                        <xsl:value-of select="concat(company, ' | ', location, ' | ', dates)"/>
                    </text>
                    <xsl:for-each select="responsibilities">
                        <text style="list-item">
                            <xsl:value-of select="concat('- ', .)" />
                        </text>
                    </xsl:for-each>
                </container>
            </xsl:for-each>

            <!-- Skills Section -->
            <text style="section-title">Technical Skills</text>
            <xsl:for-each select="/skills">
                <container style="body-text">
                    <text>
                        <xsl:value-of select="category"/>:&#160;<xsl:value-of select="list"/>
                    </text>
                </container>
            </xsl:for-each>

            <!-- Education Section -->
            <text style="section-title">Education</text>
            <xsl:for-each select="/education">
                <container style="body-text">
                    <text style="job-title"><xsl:value-of select="degree"/></text>
                    <text style="job-meta">
                        <xsl:value-of select="concat(institution, ' | ', dates)"/>
                    </text>
                </container>
            </xsl:for-each>

            <!-- Projects Section -->
            <text style="section-title">Projects</text>
            <xsl:for-each select="/projects">
                <container style="body-text">
                    <text style="job-title"><xsl:value-of select="name"/></text>
                    <text style="job-meta"><xsl:value-of select="technologies"/></text>
                    <text style="body-text"><xsl:value-of select="description"/></text>
                </container>
            </xsl:for-each>

        </page-sequence>
    </xsl:template>

</xsl:stylesheet>