<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <!-- All style definitions are the same -->
    <fo:simple-page-master page-width="8.5in" page-height="11in" margin="0.75in"/>
    <xsl:attribute-set name="header"><xsl:attribute name="padding-bottom">12pt</xsl:attribute><xsl:attribute name="border-bottom">1pt solid #ddd</xsl:attribute><xsl:attribute name="margin-bottom">16pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="name"><xsl:attribute name="font-size">28pt</xsl:attribute><xsl:attribute name="font-weight">bold</xsl:attribute><xsl:attribute name="color">#111</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="title"><xsl:attribute name="font-size">14pt</xsl:attribute><xsl:attribute name="color">#555</xsl:attribute><xsl:attribute name="margin-top">4pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="contact-info"><xsl:attribute name="text-align">right</xsl:attribute><xsl:attribute name="color">#444</xsl:attribute><xsl:attribute name="font-size">10pt</xsl:attribute><xsl:attribute name="line-height">14pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="link-color"><xsl:attribute name="color">#0066cc</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="h2"><xsl:attribute name="font-size">14pt</xsl:attribute><xsl:attribute name="font-weight">bold</xsl:attribute><xsl:attribute name="color">#333</xsl:attribute><xsl:attribute name="margin-top">12pt</xsl:attribute><xsl:attribute name="padding-bottom">4pt</xsl:attribute><xsl:attribute name="border-bottom">1px solid #eee</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="job-header"><xsl:attribute name="font-size">11pt</xsl:attribute><xsl:attribute name="margin-top">10pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="job-title"><xsl:attribute name="font-weight">bold</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="job-dates"><xsl:attribute name="text-align">right</xsl:attribute><xsl:attribute name="color">#666</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="list"><xsl:attribute name="margin-left">15pt</xsl:attribute><xsl:attribute name="margin-top">6pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="skill-category"><xsl:attribute name="font-weight">bold</xsl:attribute><xsl:attribute name="width">120pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="project-name"><xsl:attribute name="font-weight">bold</xsl:attribute><xsl:attribute name="font-size">11pt</xsl:attribute></xsl:attribute-set>


    <!-- ===================== -->
    <!-- ==== Main Template ==== -->
    <!-- ===================== -->
    <xsl:template match="/">
        <page-sequence>
            <!-- Header Section -->
            <flex-container style="header">
                <block width="65%">
                    <block style="name"><xsl:value-of select="name"/></block>
                    <block style="title"><xsl:value-of select="title"/></block>
                </block>
                <block style="contact-info" width="35%">
                    <xsl:value-of select="email"/><br/>
                    <xsl:value-of select="phone"/><br/>
                    <xsl:value-of select="location"/><br/>
                    <link href="{{linkedin}}" style="link-color"><xsl:value-of select="linkedin"/></link><br/>
                    <link href="{{github}}" style="link-color"><xsl:value-of select="github"/></link>
                </block>
            </flex-container>

            <!-- Summary Section -->
            <block style="h2">Summary</block>
            <text><xsl:value-of select="summary"/></text>

            <!-- Experience Section -->
            <block style="h2">Experience</block>
            <xsl:for-each select="experience">
                <flex-container style="job-header">
                    <block width="70%">
                        <text>
                            <strong style="job-title"><xsl:value-of select="title"/></strong>
                            <xsl:value-of select="concat(' at ', company)"/>
                        </text>
                    </block>
                    <block style="job-dates" width="30%"><xsl:value-of select="dates"/></block>
                </flex-container>
                <list style="list">
                    <xsl:for-each select="responsibilities">
                        <list-item><text><xsl:value-of select="."/></text></list-item>
                    </xsl:for-each>
                </list>
            </xsl:for-each>

            <!-- Skills Section -->
            <block style="h2">Skills</block>
            <xsl:for-each select="skills">
                <flex-container margin-top="4pt">
                    <block style="skill-category">
                        <text><xsl:value-of select="concat(category, ':')"/></text>
                    </block>
                    <block>
                        <text><xsl:value-of select="list"/></text>
                    </block>
                </flex-container>
            </xsl:for-each>

            <!-- Projects Section -->
            <block style="h2">Projects</block>
            <xsl:for-each select="projects">
                <block margin-top="8pt">
                    <flex-container>
                        <block style="project-name" width="70%"><xsl:value-of select="name"/></block>
                        <block style="job-dates" width="30%"><link href="{{url}}" style="link-color"><xsl:value-of select="url"/></link></block>
                    </flex-container>
                    <text margin-top="2pt"><xsl:value-of select="description"/></text>
                </block>
            </xsl:for-each>

            <!-- Education Section -->
            <block style="h2">Education</block>
            <xsl:for-each select="education">
                <flex-container style="job-header">
                    <block width="70%">
                        <text><strong style="job-title"><xsl:value-of select="degree"/></strong></text>
                    </block>
                    <block style="job-dates" width="30%"><xsl:value-of select="dates"/></block>
                </flex-container>
                <text margin-left="2pt"><xsl:value-of select="institution"/></text>
            </xsl:for-each>
        </page-sequence>
    </xsl:template>
</xsl:stylesheet>