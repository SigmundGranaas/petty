<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <!-- ======================================================= -->
    <!-- ==== Stylesheet Definitions (Attribute Sets & Masters) ==== -->
    <!-- ======================================================= -->

    <fo:simple-page-master page-width="8.5in" page-height="11in" margin="0.75in"/>

    <!-- General & Header -->
    <xsl:attribute-set name="header"><xsl:attribute name="padding-bottom">12pt</xsl:attribute><xsl:attribute name="border-bottom">1pt solid #ddd</xsl:attribute><xsl:attribute name="margin-bottom">16pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="name"><xsl:attribute name="font-size">28pt</xsl:attribute><xsl:attribute name="font-weight">bold</xsl:attribute><xsl:attribute name="color">#111</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="title"><xsl:attribute name="font-size">14pt</xsl:attribute><xsl:attribute name="color">#555</xsl:attribute><xsl:attribute name="margin-top">4pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="contact-info"><xsl:attribute name="text-align">right</xsl:attribute><xsl:attribute name="color">#444</xsl:attribute><xsl:attribute name="font-size">10pt</xsl:attribute><xsl:attribute name="line-height">8pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="contact-line"><xsl:attribute name="margin-top">2pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="link-color"><xsl:attribute name="color">#0066cc</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="h2"><xsl:attribute name="font-size">14pt</xsl:attribute><xsl:attribute name="font-weight">bold</xsl:attribute><xsl:attribute name="color">#333</xsl:attribute><xsl:attribute name="margin-top">12pt</xsl:attribute><xsl:attribute name="margin-bottom">4pt</xsl:attribute><xsl:attribute name="border-bottom">1px solid #eee</xsl:attribute></xsl:attribute-set>

    <!-- Experience & Education -->
    <xsl:attribute-set name="job-header"><xsl:attribute name="font-size">11pt</xsl:attribute><xsl:attribute name="margin-top">10pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="job-title"><xsl:attribute name="font-weight">bold</xsl:attribute><xsl:attribute name="margin-top">4pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="job-dates"><xsl:attribute name="text-align">right</xsl:attribute><xsl:attribute name="color">#666</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="institution-text"><xsl:attribute name="margin-left">2pt</xsl:attribute><xsl:attribute name="margin-top">4pt</xsl:attribute><xsl:attribute name="margin-bottom">4pt</xsl:attribute></xsl:attribute-set>

    <!-- Lists -->
    <xsl:attribute-set name="list"><xsl:attribute name="margin-left">15pt</xsl:attribute><xsl:attribute name="margin-top">6pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="indented-list-item"><xsl:attribute name="margin-bottom">4pt</xsl:attribute></xsl:attribute-set>

    <!-- Skills & Projects -->
    <xsl:attribute-set name="skill-category"><xsl:attribute name="font-weight">bold</xsl:attribute><xsl:attribute name="width">120pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="project"><xsl:attribute name="margin-top">8pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="project-name"><xsl:attribute name="font-weight">bold</xsl:attribute><xsl:attribute name="font-size">11pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="project-description"><xsl:attribute name="margin-top">2pt</xsl:attribute></xsl:attribute-set>

    <!-- ============================================= -->
    <!-- ==== Main Template (Document Root Match) ==== -->
    <!-- ============================================= -->
    <xsl:template match="/">
        <xsl:apply-templates select="*"/>
    </xsl:template>

    <xsl:template match="/*">
        <fo:block>
            <!-- Header Section -->
            <flex-container use-attribute-sets="header">
                <fo:block width="65%">
                    <fo:block use-attribute-sets="name"><xsl:value-of select="name"/></fo:block>
                    <fo:block use-attribute-sets="title"><xsl:value-of select="title"/></fo:block>
                </fo:block>
                <fo:block use-attribute-sets="contact-info" width="35%">
                    <fo:block use-attribute-sets="contact-line"><xsl:value-of select="email"/></fo:block>
                    <fo:block use-attribute-sets="contact-line"><xsl:value-of select="phone"/></fo:block>
                    <fo:block use-attribute-sets="contact-line"><xsl:value-of select="location"/></fo:block>
                    <fo:block use-attribute-sets="contact-line">
                        <a href="{linkedin}" use-attribute-sets="link-color"><xsl:value-of select="linkedin"/></a>
                    </fo:block>
                    <fo:block use-attribute-sets="contact-line">
                        <a href="{github}" use-attribute-sets="link-color"><xsl:value-of select="github"/></a>
                    </fo:block>
                </fo:block>
            </flex-container>

            <!--
              FIX: Apply templates for each section individually to enforce a specific order.
              The union operator `|` processes nodes in document order, which is alphabetical
              for the JSON-to-XML conversion, leading to incorrect section ordering.
            -->
            <xsl:apply-templates select="summary"/>
            <xsl:apply-templates select="experience"/>
            <xsl:apply-templates select="projects"/>
            <xsl:apply-templates select="skills"/>
            <xsl:apply-templates select="education"/>
        </fo:block>
    </xsl:template>

    <!-- ======================== -->
    <!-- ==== Section Templates ==== -->
    <!-- ======================== -->

    <xsl:template match="summary">
        <fo:block use-attribute-sets="h2">Summary</fo:block>
        <p><xsl:value-of select="."/></p>
    </xsl:template>

    <xsl:template match="experience">
        <fo:block use-attribute-sets="h2">Experience</fo:block>
        <xsl:apply-templates select="item"/>
    </xsl:template>

    <xsl:template match="skills">
        <fo:block use-attribute-sets="h2">Skills</fo:block>
        <xsl:apply-templates select="item"/>
    </xsl:template>

    <!-- FIX: Match the specific <item> child of <skills>. -->
    <xsl:template match="skills/item">
        <flex-container margin-top="4pt">
            <fo:block use-attribute-sets="skill-category">
                <p><xsl:value-of select="category"/>:</p>
            </fo:block>
            <fo:block><p><xsl:value-of select="list"/></p></fo:block>
        </flex-container>
    </xsl:template>

    <xsl:template match="projects">
        <fo:block use-attribute-sets="h2">Projects</fo:block>
        <!-- FIX: Select <item> children. -->
        <xsl:apply-templates select="item"/>
    </xsl:template>

    <!-- FIX: Match the specific <item> child of <projects>. -->
    <xsl:template match="projects/item">
        <fo:block use-attribute-sets="project">
            <flex-container>
                <fo:block use-attribute-sets="project-name" width="70%"><xsl:value-of select="name"/></fo:block>
                <fo:block use-attribute-sets="job-dates" width="30%">
                    <a href="{url}" use-attribute-sets="link-color">
                        <xsl:value-of select="url"/>
                    </a>
                </fo:block>
            </flex-container>
            <p use-attribute-sets="project-description"><xsl:value-of select="description"/></p>
        </fo:block>
    </xsl:template>

    <xsl:template match="education">
        <fo:block use-attribute-sets="h2">Education</fo:block>
        <!-- FIX: Select <item> children. -->
        <xsl:apply-templates select="item"/>
    </xsl:template>

    <!-- FIX: Match the specific <item> child of <experience>. -->
    <xsl:template match="experience/item">
        <flex-container use-attribute-sets="job-header">
            <fo:block width="70%">
                <p>
                    <span use-attribute-sets="job-title"><xsl:value-of select="title"/></span>, <xsl:value-of select="company"/>
                </p>
            </fo:block>
            <fo:block use-attribute-sets="job-dates" width="30%">
                <p><xsl:value-of select="dates"/></p>
            </fo:block>
        </flex-container>
        <list use-attribute-sets="list">
            <xsl:for-each select="responsibilities/item">
                <list-item use-attribute-sets="indented-list-item">
                    <p><xsl:value-of select="."/></p>
                </list-item>
            </xsl:for-each>
        </list>
    </xsl:template>

    <!-- FIX: Match the specific <item> child of <education>. -->
    <xsl:template match="education/item">
        <flex-container use-attribute-sets="job-header">
            <fo:block width="70%">
                <p>
                    <span use-attribute-sets="job-title"><xsl:value-of select="degree"/></span>
                </p>
            </fo:block>
            <fo:block use-attribute-sets="job-dates" width="30%">
                <p><xsl:value-of select="dates"/></p>
            </fo:block>
        </flex-container>
        <p use-attribute-sets="institution-text"><xsl:value-of select="institution"/></p>
    </xsl:template>
</xsl:stylesheet>