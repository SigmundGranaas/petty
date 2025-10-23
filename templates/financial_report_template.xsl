<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <!-- Page Layout -->
    <fo:simple-page-master page-width="8.5in" page-height="11in" margin="50pt"/>

    <!-- ============================================= -->
    <!-- Main Template (Entry Point)                   -->
    <!-- ============================================= -->
    <xsl:template match="/*">
        <fo:block>
            <p use-attribute-sets="h1"><xsl:value-of select="companyName"/></p>
            <p use-attribute-sets="h2"><xsl:value-of select="reportTitle"/></p>

            <xsl:apply-templates select="sections/item"/>
        </fo:block>
    </xsl:template>

    <xsl:template match="sections/item">
        <p use-attribute-sets="section-title"><xsl:value-of select="title"/></p>
        <xsl:apply-templates select="tables/item"/>
    </xsl:template>

    <xsl:template match="tables/item">
        <xsl:if test="title">
            <p use-attribute-sets="table-title"><xsl:value-of select="title"/></p>
        </xsl:if>
        <table>
            <columns>
                <column width="70%"/>
                <column width="30%"/>
            </columns>
            <tbody>
                <!-- apply-templates will find the best match for each 'data' item -->
                <xsl:apply-templates select="data/item"/>
            </tbody>
        </table>
    </xsl:template>

    <!-- ============================================= -->
    <!-- TEMPLATES for each data row type (using match)-->
    <!-- ============================================= -->
    <xsl:template match="item[type='item']">
        <row>
            <cell use-attribute-sets="td"><p><xsl:value-of select="label"/></p></cell>
            <cell use-attribute-sets="td td-right"><p><xsl:value-of select="value"/></p></cell>
        </row>
    </xsl:template>

    <xsl:template match="item[type='subtotal']">
        <row>
            <cell use-attribute-sets="td subtotal-row-cell"><p><xsl:value-of select="label"/></p></cell>
            <cell use-attribute-sets="td td-right subtotal-row-cell"><p><xsl:value-of select="value"/></p></cell>
        </row>
    </xsl:template>

    <xsl:template match="item[type='total']">
        <row>
            <cell use-attribute-sets="td total-row-cell"><p><xsl:value-of select="label"/></p></cell>
            <cell use-attribute-sets="td td-right total-row-cell"><p><xsl:value-of select="value"/></p></cell>
        </row>
    </xsl:template>

    <!-- ============================================= -->
    <!-- Style Definitions                             -->
    <!-- ============================================= -->
    <xsl:attribute-set name="h1"><xsl:attribute name="font-size">32pt</xsl:attribute><xsl:attribute name="font-weight">bold</xsl:attribute><xsl:attribute name="color">#142850</xsl:attribute><xsl:attribute name="margin-bottom">10pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="h2"><xsl:attribute name="font-size">24pt</xsl:attribute><xsl:attribute name="color">#27496d</xsl:attribute><xsl:attribute name="margin-bottom">40pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="section-title"><xsl:attribute name="font-size">18pt</xsl:attribute><xsl:attribute name="font-weight">bold</xsl:attribute><xsl:attribute name="color">#142850</xsl:attribute><xsl:attribute name="border-bottom">1pt solid #142850</xsl:attribute><xsl:attribute name="padding-bottom">4pt</xsl:attribute><xsl:attribute name="margin-top">20pt</xsl:attribute><xsl:attribute name="margin-bottom">15pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="table-title"><xsl:attribute name="font-size">14pt</xsl:attribute><xsl:attribute name="font-weight">bold</xsl:attribute><xsl:attribute name="margin-bottom">10pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="td"><xsl:attribute name="font-size">11pt</xsl:attribute><xsl:attribute name="padding-top">6pt</xsl:attribute><xsl:attribute name="padding-bottom">6pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="td-right" use-attribute-sets="td"><xsl:attribute name="text-align">right</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="subtotal-row-cell"><xsl:attribute name="font-weight">bold</xsl:attribute><xsl:attribute name="border-top">0.5pt solid #888888</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="total-row-cell"><xsl:attribute name="font-weight">bold</xsl:attribute><xsl:attribute name="border-top">1.5pt solid #142850</xsl:attribute></xsl:attribute-set>

</xsl:stylesheet>