<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform" xmlns:fo="http://www.w3.org/1999/XSL/Format">
    <!-- Page Layout Definition -->
    <fo:simple-page-master page-width="8.5in" page-height="11in" margin="0.75in"/>

    <!-- Reusable Style Definitions -->
    <xsl:attribute-set name="h1">
        <xsl:attribute name="font-size">24pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="color">#2c3e50</xsl:attribute>
        <xsl:attribute name="margin-bottom">4pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="h2">
        <xsl:attribute name="font-size">14pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="color">#2c3e50</xsl:attribute>
        <xsl:attribute name="margin-top">12pt</xsl:attribute>
        <xsl:attribute name="margin-bottom">8pt</xsl:attribute>
        <xsl:attribute name="padding-bottom">4pt</xsl:attribute>
        <xsl:attribute name="border-bottom">1pt solid #bdc3c7</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="h3">
        <xsl:attribute name="font-size">11pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="italic">
        <xsl:attribute name="font-style">italic</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="subtle">
        <xsl:attribute name="color">#7f8c8d</xsl:attribute>
    </xsl:attribute-set>

    <!-- Main Template -->
    <xsl:template match="/">
        <page-sequence select=".">
            <!-- Header Section -->
            <container>
                <text style="h1"><xsl:value-of select="name"/></text>
                <text font-size="12pt" color="#34495e" margin-bottom="8pt"><xsl:value-of select="title"/></text>
                <flex-container font-size="9pt" color="#34495e">
                    <container width="33%"><text><xsl:value-of select="email"/></text></container>
                    <container width="33%"><text text-align="center"><xsl:value-of select="phone"/></text></container>
                    <container width="33%"><text text-align="right"><xsl:value-of select="location"/></text></container>
                </flex-container>
                <flex-container font-size="9pt">
                    <container width="50%"><text>
                        <link href="https://{linkedin}"><xsl:value-of select="linkedin"/></link>
                    </text></container>
                    <container width="50%"><text text-align="right">
                        <link href="https://{github}"><xsl:value-of select="github"/></link>
                    </text></container>
                </flex-container>
            </container>

            <!-- Summary Section -->
            <container margin-top="16pt">
                <text style="h2">Summary</text>
                <text font-size="10pt" line-height="14pt"><xsl:value-of select="summary"/></text>
            </container>

            <!-- Experience Section -->
            <container>
                <text style="h2">Experience</text>
                <xsl:for-each select="experience">
                    <container margin-bottom="12pt">
                        <flex-container>
                            <container width="75%"><text style="h3"><xsl:value-of select="title"/> @ <xsl:value-of select="company"/></text></container>
                            <container width="25%"><text font-size="10pt" text-align="right" style="subtle"><xsl:value-of select="dates"/></text></container>
                        </flex-container>
                        <text font-size="10pt" style="subtle" margin-bottom="4pt"><xsl:value-of select="location"/></text>
                        <list font-size="10pt" line-height="14pt" padding-left="15pt">
                            <xsl:for-each select="responsibilities">
                                <list-item><text><xsl:value-of select="."/></text></list-item>
                            </xsl:for-each>
                        </list>
                    </container>
                </xsl:for-each>
            </container>

            <!-- Skills Section -->
            <container>
                <text style="h2">Skills</text>
                <xsl:for-each select="skills">
                    <text font-size="10pt" line-height="14pt">
                        <strong font-weight="bold"><xsl:value-of select="category"/>: </strong>
                        <xsl:value-of select="list"/>
                    </text>
                </xsl:for-each>
            </container>

            <!-- Projects Section -->
            <container>
                <text style="h2">Projects</text>
                <xsl:for-each select="projects">
                    <container margin-bottom="8pt">
                        <text font-size="10pt">
                            <strong font-weight="bold"><xsl:value-of select="name"/></strong> -
                            <em style="italic" color="#3498db"><xsl:value-of select="url"/></em>
                        </text>
                        <text font-size="10pt"><xsl:value-of select="description"/></text>
                    </container>
                </xsl:for-each>
            </container>

            <!-- Education Section -->
            <container>
                <text style="h2">Education</text>
                <xsl:for-each select="education">
                    <flex-container margin-bottom="4pt">
                        <container width="75%"><text style="h3"><xsl:value-of select="institution"/></text></container>
                        <container width="25%"><text font-size="10pt" text-align="right" style="subtle"><xsl:value-of select="dates"/></text></container>
                    </flex-container>
                    <text font-size="10pt" margin-bottom="8pt"><xsl:value-of select="degree"/></text>
                </xsl:for-each>
            </container>
        </page-sequence>
    </xsl:template>
</xsl:stylesheet>