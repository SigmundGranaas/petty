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
    <xsl:attribute-set name="link-color"><xsl:attribute name="color">#0066cc</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="h2"><xsl:attribute name="font-size">14pt</xsl:attribute><xsl:attribute name="font-weight">bold</xsl:attribute><xsl:attribute name="color">#333</xsl:attribute><xsl:attribute name="margin-top">12pt</xsl:attribute><xsl:attribute name="margin-bottom">4pt</xsl:attribute><xsl:attribute name="border-bottom">1px solid #eee</xsl:attribute></xsl:attribute-set>

    <!-- Experience & Education -->
    <xsl:attribute-set name="job-header"><xsl:attribute name="font-size">11pt</xsl:attribute><xsl:attribute name="margin-top">10pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="job-title"><xsl:attribute name="font-weight">bold</xsl:attribute><xsl:attribute name="margin-top">4pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="job-dates"><xsl:attribute name="text-align">right</xsl:attribute><xsl:attribute name="color">#666</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="institution-text"><xsl:attribute name="margin-left">2pt</xsl:attribute><xsl:attribute name="margin-top">4pt</xsl:attribute><xsl:attribute name="margin-bottom">4pt</xsl:attribute></xsl:attribute-set>

    <!-- Lists -->
    <xsl:attribute-set name="list"><xsl:attribute name="margin-left">15pt</xsl:attribute><xsl:attribute name="margin-top">6pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="list-item"><xsl:attribute name="margin-top">6pt</xsl:attribute></xsl:attribute-set>
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
        <!-- The root node of the data source has a single child element, e.g., <root> -->
        <xsl:apply-templates select="*"/>
    </xsl:template>

    <xsl:template match="root">
        <page-sequence>
            <!-- Header Section -->
            <flex-container use-attribute-sets="header">
                <block width="65%">
                    <block use-attribute-sets="name"><xsl:value-of select="name"/></block>
                    <block use-attribute-sets="title"><xsl:value-of select="title"/></block>
                </block>
                <block use-attribute-sets="contact-info" width="35%">
                    <xsl:value-of select="email"/><br/>
                    <xsl:value-of select="phone"/><br/>
                    <xsl:value-of select="location"/><br/>
                    <!-- NOTE: Dynamic href requires <xsl:attribute>, which is not yet implemented. Using a placeholder. -->
                    <link href="#" use-attribute-sets="link-color"><xsl:value-of select="linkedin"/></link><br/>
                    <link href="#" use-attribute-sets="link-color"><xsl:value-of select="github"/></link>
                </block>
            </flex-container>

            <!-- Process all direct children of the <root> element in a specific order -->
            <xsl:apply-templates select="summary | experience | skills | projects | education"/>
        </page-sequence>
    </xsl:template>

    <!-- ======================== -->
    <!-- ==== Section Templates ==== -->
    <!-- ======================== -->

    <xsl:template match="summary">
        <block use-attribute-sets="h2">Summary</block>
        <p><xsl:value-of select="."/></p>
    </xsl:template>

    <xsl:template match="experience">
        <block use-attribute-sets="h2">Experience</block>
        <xsl:apply-templates select="job"/>
    </xsl:template>

    <xsl:template match="skills">
        <block use-attribute-sets="h2">Skills</block>
        <xsl:apply-templates select="skill"/>
    </xsl:template>

    <xsl:template match="skill">
        <flex-container margin-top="4pt">
            <block use-attribute-sets="skill-category">
                <p><xsl:value-of select="category"/><xsl:text>:</xsl:text></p>
            </block>
            <block><p><xsl:value-of select="list"/></p></block>
        </flex-container>
    </xsl:template>

    <xsl:template match="projects">
        <block use-attribute-sets="h2">Projects</block>
        <xsl:apply-templates select="project"/>
    </xsl:template>

    <xsl:template match="project">
        <block use-attribute-sets="project">
            <flex-container>
                <block use-attribute-sets="project-name" width="70%"><xsl:value-of select="name"/></block>
                <block use-attribute-sets="job-dates" width="30%">
                    <!-- NOTE: Dynamic href requires <xsl:attribute>, which is not yet implemented. Using a placeholder. -->
                    <link href="#" use-attribute-sets="link-color">
                        <xsl:value-of select="url"/>
                    </link>
                </block>
            </flex-container>
            <p use-attribute-sets="project-description"><xsl:value-of select="description"/></p>
        </block>
    </xsl:template>

    <xsl:template match="education">
        <block use-attribute-sets="h2">Education</block>
        <xsl:apply-templates select="entry"/>
    </xsl:template>


    <xsl:template match="job">
        <flex-container use-attribute-sets="job-header">
            <block width="70%">
                <p>
                    <span use-attribute-sets="job-title"><xsl:value-of select="title"/></span>
                    <xsl:text> </xsl:text>
                    <xsl:value-of select="company"/>
                </p>
            </block>
            <block use-attribute-sets="job-dates" width="30%">
                <p><xsl:value-of select="dates"/></p>
            </block>
        </flex-container>
        <list use-attribute-sets="list">
            <xsl:for-each select="responsibilities/item">
                <list-item use-attribute-sets="indented-list-item">
                    <p><xsl:value-of select="."/></p>
                </list-item>
            </xsl:for-each>
        </list>
    </xsl:template>

    <xsl:template match="entry">
        <flex-container use-attribute-sets="job-header">
            <block width="70%">
                <p>
                    <span use-attribute-sets="job-title"><xsl:value-of select="degree"/></span>
                </p>
            </block>
            <block use-attribute-sets="job-dates" width="30%">
                <p><xsl:value-of select="dates"/></p>
            </block>
        </flex-container>
        <p use-attribute-sets="institution-text"><xsl:value-of select="institution"/></p>
    </xsl:template>
</xsl:stylesheet>